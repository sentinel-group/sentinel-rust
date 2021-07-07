use super::{BlockError, EntryContext, TokenResult, SLOT_INIT};
use crate::logging::log::error;
use std::any;
use std::cell::RefCell;
use std::rc::Rc;

// trait for upcast/downcast
pub trait AsAny: any::Any {
    fn as_any(&self) -> &dyn any::Any;
}

// impl the required AsAny trait for structs with BaseSlot triat
// BaseSlot bound is necessary, Otherwise, sequential downcasting is invalid
impl<T: BaseSlot> AsAny for T {
    fn as_any(&self) -> &dyn any::Any {
        self
    }
}

/// trait PartialOrd is not object safe
/// SlotChain will sort all it's slots by ascending sort value in each bucket
/// (StatPrepareSlot bucket、RuleCheckSlot bucket and StatSlot bucket)
pub trait BaseSlot: AsAny {
    /// order returns the sort value of the slot.
    fn order(&self) -> u32 {
        0
    }
}

/// StatPrepareSlot is responsible for some preparation before statistic
/// For example: init structure and so on
pub trait StatPrepareSlot: BaseSlot {
    /// prepare fntion do some initialization
    /// Such as: init statistic structure、node and etc
    /// The result of preparing would store in EntryContext
    /// All StatPrepareSlots execute in sequence
    /// prepare fntion should not throw panic.
    fn prepare(&self, ctx: Rc<RefCell<EntryContext>>) {}
}

/// RuleCheckSlot is rule based checking strategy
/// All checking rule must implement this interface.
pub trait RuleCheckSlot: BaseSlot {
    // check fntion do some validation
    // It can break off the slot pipeline
    // Each TokenResult will return check result
    // The upper logic will control pipeline according to SlotResult.
    fn check(&self, ctx: Rc<RefCell<EntryContext>>) -> TokenResult {
        TokenResult::new_pass()
    }
}

/// StatSlot is responsible for counting all custom biz metrics.
/// StatSlot would not handle any panic, and pass up all panic to slot chain
pub trait StatSlot: BaseSlot {
    /// OnEntryPass fntion will be invoked when StatPrepareSlots and RuleCheckSlots execute pass
    /// StatSlots will do some statistic logic, such as QPS、log、etc
    fn on_entry_pass(&self, ctx: Rc<RefCell<EntryContext>>) {}
    /// on_entry_blocked fntion will be invoked when StatPrepareSlots and RuleCheckSlots fail to execute
    /// It may be inbound flow control or outbound cir
    /// StatSlots will do some statistic logic, such as QPS、log、etc
    /// blockError introduce the block detail
    fn on_entry_blocked(&self, ctx: Rc<RefCell<EntryContext>>, block_error: Option<BlockError>) {}
    /// on_completed fntion will be invoked when chain exits.
    /// The semantics of on_completed is the entry passed and completed
    /// Note: blocked entry will not call this fntion
    fn on_completed(&self, ctx: Rc<RefCell<EntryContext>>) {}
}

/// SlotChain hold all system slots and customized slot.
/// SlotChain support plug-in slots developed by developer.
pub struct SlotChain {
    /// statPres is in ascending order by StatPrepareSlot.order() value.
    pub(crate) stat_pres: Vec<Rc<dyn StatPrepareSlot>>,
    /// ruleChecks is in ascending order by RuleCheckSlot.order() value.
    pub(crate) rule_checks: Vec<Rc<dyn RuleCheckSlot>>,
    /// stats is in ascending order by StatSlot.order() value.
    pub(crate) stats: Vec<Rc<dyn StatSlot>>,
}

impl SlotChain {
    pub fn new() -> Self {
        Self {
            stat_pres: Vec::with_capacity(SLOT_INIT),
            rule_checks: Vec::with_capacity(SLOT_INIT),
            stats: Vec::with_capacity(SLOT_INIT),
        }
    }
    pub fn exit(&self, ctx: Rc<RefCell<EntryContext>>) {
        if ctx.borrow().entry().is_none() {
            error!("SentinelEntry is nil in SlotChain.exit()");
            return;
        }
        if ctx.borrow().is_blocked() {
            return;
        }
        // The on_completed is called only when entry passed
        for s in &self.stats {
            s.on_completed(ctx.clone());
        }
    }

    /// add_stat_prepare_slot adds the StatPrepareSlot slot to the StatPrepareSlot list of the SlotChain.
    /// All StatPrepareSlot in the list will be sorted according to StatPrepareSlot.order() in ascending order.
    /// add_stat_prepare_slot is non-thread safe,
    /// In concurrency scenario, add_stat_prepare_slot must be guarded by SlotChain.RWMutex#Lock
    pub fn add_stat_prepare_slot(&mut self, s: Rc<dyn StatPrepareSlot>) {
        self.stat_pres.push(s);
        self.stat_pres.sort_unstable_by_key(|a| a.order());
    }

    // add_rule_check_slot adds the RuleCheckSlot to the RuleCheckSlot list of the SlotChain.
    // All RuleCheckSlot in the list will be sorted according to RuleCheckSlot.order() in ascending order.
    // add_rule_check_slot is non-thread safe,
    // In concurrency scenario, add_rule_check_slot must be guarded by SlotChain.RWMutex#Lock
    pub fn add_rule_check_slot(&mut self, s: Rc<dyn RuleCheckSlot>) {
        self.rule_checks.push(s);
        self.rule_checks.sort_unstable_by_key(|a| a.order());
    }

    // add_stat_slot adds the StatSlot to the StatSlot list of the SlotChain.
    // All StatSlot in the list will be sorted according to StatSlot.order() in ascending order.
    // add_stat_slot is non-thread safe,
    // In concurrency scenario, add_stat_slot must be guarded by SlotChain.RWMutex#Lock
    pub fn add_stat_slot(&mut self, s: Rc<dyn StatSlot>) {
        self.stats.push(s);
        self.stats.sort_unstable_by_key(|a| a.order());
    }

    /// The entrance of slot chain
    /// Return the TokenResult
    pub fn entry(&self, ctx: Rc<RefCell<EntryContext>>) -> TokenResult {
        // execute prepare slot
        for s in &self.stat_pres {
            s.prepare(ctx.clone());
        }

        // execute rule based checking slot
        ctx.borrow_mut().rule_check_result.reset_to_pass();
        for s in &self.rule_checks {
            let res = s.check(ctx.clone());
            // check slot result
            if res.is_blocked() {
                ctx.borrow_mut().rule_check_result = res;
            }
        }

        // execute statistic slot
        for s in &self.stats {
            // indicate the result of rule based checking slot.
            if ctx.borrow_mut().rule_check_result.is_pass() {
                s.on_entry_pass(ctx.clone())
            } else {
                // The block error should not be nil.
                s.on_entry_blocked(
                    ctx.clone(),
                    ctx.borrow().rule_check_result.block_err.clone(),
                )
            }
        }
        ctx.borrow().rule_check_result.clone()
    }
}

#[cfg(test)]
pub(crate) use test::aggregation::*;

#[cfg(test)]
mod test {
    use super::super::{
        BlockType, ConcurrencyStat, MetricItem, MetricItemRetriever, MockStatNode, NopReadStat,
        ReadStat, ResourceType, ResourceWrapper, SentinelEntry, SentinelInput, TimePredicate,
        TokenResultStatus, TrafficType, WriteStat,
    };
    use super::*;
    use crate::Result;

    // here we test three kinds of slots one by one
    mod single {
        use super::*;
        struct StatPrepareSlotMock {
            pub(crate) name: String,
            pub(crate) order: u32,
        }
        impl BaseSlot for StatPrepareSlotMock {
            fn order(&self) -> u32 {
                self.order
            }
        }
        impl StatPrepareSlot for StatPrepareSlotMock {}
        #[test]
        fn add_stat_prepare_slot() {
            let mut sc = SlotChain::new();
            for base in &[2, 1, 3, 0, 4] {
                for i in 0..10 {
                    let order = base * 10 + i;
                    sc.add_stat_prepare_slot(Rc::new(StatPrepareSlotMock {
                        name: String::from(format!("mock{}", order)),
                        order,
                    }))
                }
            }
            assert_eq!(sc.stat_pres.len(), 50);
            let mut idx = -1;
            for (i, s) in sc.stat_pres.into_iter().map(|slot| {
                idx += 1;
                (idx, slot)
            }) {
                assert_eq!(
                    s.as_any()
                        .downcast_ref::<StatPrepareSlotMock>()
                        .unwrap()
                        .name,
                    format!("mock{}", i)
                );
            }
        }

        struct RuleCheckSlotMock {
            name: String,
            order: u32,
        }
        impl BaseSlot for RuleCheckSlotMock {
            fn order(&self) -> u32 {
                self.order
            }
        }
        impl RuleCheckSlot for RuleCheckSlotMock {}
        #[test]
        fn add_rule_check_slot() {
            let mut sc = SlotChain::new();
            for base in &[2, 1, 3, 0, 4] {
                for i in 0..10 {
                    let order = base * 10 + i;
                    sc.add_rule_check_slot(Rc::new(RuleCheckSlotMock {
                        name: String::from(format!("mock{}", order)),
                        order,
                    }))
                }
            }
            assert_eq!(sc.rule_checks.len(), 50);
            let mut idx = -1;
            for (i, s) in sc.rule_checks.into_iter().map(|slot| {
                idx += 1;
                (idx, slot)
            }) {
                assert_eq!(
                    s.as_any().downcast_ref::<RuleCheckSlotMock>().unwrap().name,
                    format!("mock{}", i)
                );
            }
        }

        struct StatSlotMock {
            name: String,
            order: u32,
        }
        impl BaseSlot for StatSlotMock {
            fn order(&self) -> u32 {
                self.order
            }
        }
        impl StatSlot for StatSlotMock {}
        #[test]
        fn add_stat_slot() {
            let mut sc = SlotChain::new();
            for base in &[2, 1, 3, 0, 4] {
                for i in 0..10 {
                    let order = base * 10 + i;
                    sc.add_stat_slot(Rc::new(StatSlotMock {
                        name: String::from(format!("mock{}", order)),
                        order,
                    }))
                }
            }
            assert_eq!(sc.stats.len(), 50);
            let mut idx = -1;
            for (i, s) in sc.stats.into_iter().map(|slot| {
                idx += 1;
                (idx, slot)
            }) {
                assert_eq!(
                    s.as_any().downcast_ref::<StatSlotMock>().unwrap().name,
                    format!("mock{}", i)
                );
            }
        }
    }

    pub(crate) mod aggregation {
        use super::*;
        use mockall::predicate::*;
        use mockall::*;
        use std::any;

        // these signatures are necessary, don't remove them
        // because when use macro `mock!`, we have to supply the signatures expected to be mocked
        // otherwise, we cannot call `expect_xx()` on mocked objects
        mock! {
            pub(crate) StatPrepareSlot {}
            impl BaseSlot for StatPrepareSlot {}
            impl StatPrepareSlot for StatPrepareSlot { fn prepare(&self, ctx: Rc<RefCell<EntryContext>>); }
        }

        mock! {
            pub(crate) RuleCheckSlot {}
            impl BaseSlot for RuleCheckSlot {}
            impl RuleCheckSlot for RuleCheckSlot { fn check(&self, ctx: Rc<RefCell<EntryContext>>) -> TokenResult; }
        }

        mock! {
            pub(crate) StatSlot {}
            impl BaseSlot for StatSlot {}
            impl StatSlot for StatSlot {
                fn on_entry_pass(&self, ctx: Rc<RefCell<EntryContext>>);
                fn on_entry_blocked(&self, ctx: Rc<RefCell<EntryContext>>, block_error: Option<BlockError>);
                fn on_completed(&self, ctx: Rc<RefCell<EntryContext>>);
            }
        }

        #[test]
        fn pass_and_exit() {
            let sc = Rc::new(RefCell::new(SlotChain::new()));
            let ctx = Rc::new(RefCell::new(EntryContext::new()));
            let rw = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
            ctx.borrow_mut().res = Some(rw.clone());
            ctx.borrow_mut()
                .set_entry(Rc::new(RefCell::new(SentinelEntry::new(
                    Some(rw),
                    Rc::downgrade(&ctx),
                    sc.clone(),
                ))));
            ctx.borrow_mut().stat_node = Some(Rc::new(RefCell::new(MockStatNode::new())));
            ctx.borrow_mut().input = Some(SentinelInput::new());
            let mut ps = Rc::new(MockStatPrepareSlot::new());
            let mut rcs1 = Rc::new(MockRuleCheckSlot::new());
            let mut rcs2 = Rc::new(MockRuleCheckSlot::new());
            let mut ssm = Rc::new(MockStatSlot::new());

            let mut seq = Sequence::new();
            Rc::get_mut(&mut ps)
                .unwrap()
                .expect_prepare()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Rc::get_mut(&mut rcs1)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_pass());
            Rc::get_mut(&mut rcs2)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_pass());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_pass()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_blocked()
                .never()
                .return_const(());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_completed()
                .once()
                .in_sequence(&mut seq)
                .return_const(());

            sc.borrow_mut().add_stat_prepare_slot(ps.clone());
            sc.borrow_mut().add_rule_check_slot(rcs1.clone());
            sc.borrow_mut().add_rule_check_slot(rcs2.clone());
            sc.borrow_mut().add_stat_slot(ssm.clone());

            let r = sc.borrow_mut().entry(ctx.clone());
            assert_eq!(TokenResultStatus::Pass, r.status, "should pass but blocked");
            sc.borrow_mut().exit(ctx.clone());
        }
        #[test]
        fn block() {
            let sc = Rc::new(RefCell::new(SlotChain::new()));
            let ctx = Rc::new(RefCell::new(EntryContext::new()));
            let rw = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
            ctx.borrow_mut().res = Some(rw.clone());
            ctx.borrow_mut()
                .set_entry(Rc::new(RefCell::new(SentinelEntry::new(
                    Some(rw),
                    Rc::downgrade(&ctx),
                    sc.clone(),
                ))));
            ctx.borrow_mut().stat_node = Some(Rc::new(RefCell::new(MockStatNode::new())));
            ctx.borrow_mut().input = Some(SentinelInput::new());
            let mut ps = Rc::new(MockStatPrepareSlot::new());
            let mut rcs1 = Rc::new(MockRuleCheckSlot::new());
            let mut rcs2 = Rc::new(MockRuleCheckSlot::new());
            let mut ssm = Rc::new(MockStatSlot::new());

            let mut seq = Sequence::new();
            Rc::get_mut(&mut ps)
                .unwrap()
                .expect_prepare()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Rc::get_mut(&mut rcs1)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_pass());
            Rc::get_mut(&mut rcs2)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_blocked(BlockType::Flow));
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_pass()
                .never()
                .return_const(());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_blocked()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_completed()
                .never()
                .return_const(());

            sc.borrow_mut().add_stat_prepare_slot(ps);
            sc.borrow_mut().add_rule_check_slot(rcs1);
            sc.borrow_mut().add_rule_check_slot(rcs2);
            sc.borrow_mut().add_stat_slot(ssm);

            let r = sc.borrow_mut().entry(ctx.clone());
            assert_eq!(
                TokenResultStatus::Blocked,
                r.status,
                "should blocked but pass"
            );
            assert_eq!(
                BlockType::Flow,
                r.block_err.unwrap().block_type,
                "should blocked by BlockType Flow"
            );
            sc.borrow_mut().exit(ctx.clone());
        }

        struct StatPrepareSlotBadMock {}

        impl BaseSlot for StatPrepareSlotBadMock {}

        impl StatPrepareSlot for StatPrepareSlotBadMock {
            fn prepare(&self, ctx: Rc<RefCell<EntryContext>>) {
                panic!("sentinel internal panic for test");
            }
        }
        #[test]
        #[should_panic(expected = "sentinel internal panic for test")]
        fn should_panic() {
            let sc = Rc::new(RefCell::new(SlotChain::new()));
            let ctx = Rc::new(RefCell::new(EntryContext::new()));
            let rw = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
            ctx.borrow_mut().res = Some(rw.clone());
            ctx.borrow_mut()
                .set_entry(Rc::new(RefCell::new(SentinelEntry::new(
                    Some(rw),
                    Rc::downgrade(&ctx),
                    sc.clone(),
                ))));
            ctx.borrow_mut().stat_node = Some(Rc::new(RefCell::new(MockStatNode::new())));
            ctx.borrow_mut().input = Some(SentinelInput::new());
            let ps = Rc::new(StatPrepareSlotBadMock {});
            let mut rcs1 = Rc::new(MockRuleCheckSlot::new());
            let mut rcs2 = Rc::new(MockRuleCheckSlot::new());
            let mut ssm = Rc::new(MockStatSlot::new());

            Rc::get_mut(&mut rcs1)
                .unwrap()
                .expect_check()
                .never()
                .returning(|_ctx| TokenResult::new_pass());
            Rc::get_mut(&mut rcs2)
                .unwrap()
                .expect_check()
                .never()
                .returning(|_ctx| TokenResult::new_blocked(BlockType::Flow));
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_pass()
                .never()
                .return_const(());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_blocked()
                .never()
                .return_const(());
            Rc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_completed()
                .never()
                .return_const(());

            sc.borrow_mut().add_stat_prepare_slot(ps);
            sc.borrow_mut().add_rule_check_slot(rcs1);
            sc.borrow_mut().add_rule_check_slot(rcs2);
            sc.borrow_mut().add_stat_slot(ssm);

            let r = sc.borrow_mut().entry(ctx.clone());
        }
    }
}
