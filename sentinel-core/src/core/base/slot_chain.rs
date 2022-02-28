use super::{BlockError, ContextPtr, EntryContext, TokenResult, SLOT_INIT};
use crate::logging;
use crate::utils::AsAny;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// trait `PartialOrd` is not object safe
/// SlotChain will sort all it's slots by ascending sort value in each bucket
/// (StatPrepareSlot bucket、RuleCheckSlot bucket and StatSlot bucket)
pub trait BaseSlot: Any + AsAny + Sync + Send {
    /// order returns the sort value of the slot.
    fn order(&self) -> u32 {
        0
    }
}

// todo: replace `Rc` of ctx to `&Rc` in these slots

/// StatPrepareSlot is responsible for some preparation before statistic
/// For example: init structure and so on
pub trait StatPrepareSlot: BaseSlot {
    /// prepare fntion do some initialization
    /// Such as: init statistic structure、node and etc
    /// The result of preparing would store in EntryContext
    /// All StatPrepareSlots execute in sequence
    /// prepare fntion should not throw panic.
    fn prepare(&self, ctx: ContextPtr) {}
}

/// RuleCheckSlot is rule based checking strategy
/// All checking rule must implement this interface.
pub trait RuleCheckSlot: BaseSlot {
    // check fntion do some validation
    // It can break off the slot pipeline
    // Each TokenResult will return check result
    // The upper logic will control pipeline according to SlotResult.
    fn check(&self, ctx: &ContextPtr) -> TokenResult {
        cfg_if_async! {
            let ctx = ctx.read().unwrap(),
            let ctx = ctx.borrow()
        };
        ctx.result().clone()
    }
}

/// StatSlot is responsible for counting all custom biz metrics.
/// StatSlot would not handle any panic, and pass up all panic to slot chain
pub trait StatSlot: BaseSlot {
    /// OnEntryPass fntion will be invoked when StatPrepareSlots and RuleCheckSlots execute pass
    /// StatSlots will do some statistic logic, such as QPS、log、etc
    fn on_entry_pass(&self, ctx: ContextPtr) {}
    /// on_entry_blocked fntion will be invoked when StatPrepareSlots and RuleCheckSlots fail to execute
    /// It may be inbound flow control or outbound cir
    /// StatSlots will do some statistic logic, such as QPS、log、etc
    /// blockError introduce the block detail
    fn on_entry_blocked(&self, ctx: ContextPtr, block_error: Option<BlockError>) {}
    /// on_completed fntion will be invoked when chain exits.
    /// The semantics of on_completed is the entry passed and completed
    /// Note: blocked entry will not call this fntion
    fn on_completed(&self, ctx: ContextPtr) {}
}

/// SlotChain hold all system slots and customized slot.
/// SlotChain support plug-in slots developed by developer.
pub struct SlotChain {
    /// statPres is in ascending order by StatPrepareSlot.order() value.
    pub(self) stat_pres: Vec<Arc<dyn StatPrepareSlot>>,
    /// ruleChecks is in ascending order by RuleCheckSlot.order() value.
    pub(self) rule_checks: Vec<Arc<dyn RuleCheckSlot>>,
    /// stats is in ascending order by StatSlot.order() value.
    pub(self) stats: Vec<Arc<dyn StatSlot>>,
}

impl SlotChain {
    pub fn new() -> Self {
        Self {
            stat_pres: Vec::with_capacity(SLOT_INIT),
            rule_checks: Vec::with_capacity(SLOT_INIT),
            stats: Vec::with_capacity(SLOT_INIT),
        }
    }

    cfg_async! {
        pub fn exit(&self, ctx: ContextPtr) {
            if ctx.read().unwrap().entry().is_none() {
                logging::error!("SentinelEntry is nil in SlotChain.exit()");
                return;
            }
            if ctx.read().unwrap().is_blocked() {
                return;
            }
            // The on_completed is called only when entry passed
            for s in &self.stats {
                s.on_completed(ctx.clone()); // Rc/Arc clone
            }
        }
    }

    cfg_not_async! {
        pub fn exit(&self, ctx: ContextPtr) {
            if ctx.borrow().entry().is_none() {
                logging::error!("SentinelEntry is nil in SlotChain.exit()");
                return;
            }
            if ctx.borrow().is_blocked() {
                return;
            }
            // The on_completed is called only when entry passed
            for s in &self.stats {
                s.on_completed(ctx.clone()); // Rc/Arc clone
            }
        }
    }

    /// add_stat_prepare_slot adds the StatPrepareSlot slot to the StatPrepareSlot list of the SlotChain.
    /// All StatPrepareSlot in the list will be sorted according to StatPrepareSlot.order() in ascending order.
    /// add_stat_prepare_slot is non-thread safe,
    /// In concurrency scenario, add_stat_prepare_slot must be guarded by SlotChain.RWMutex#Lock
    pub fn add_stat_prepare_slot(&mut self, s: Arc<dyn StatPrepareSlot>) {
        self.stat_pres.push(s);
        self.stat_pres.sort_unstable_by_key(|a| a.order());
    }

    // add_rule_check_slot adds the RuleCheckSlot to the RuleCheckSlot list of the SlotChain.
    // All RuleCheckSlot in the list will be sorted according to RuleCheckSlot.order() in ascending order.
    // add_rule_check_slot is non-thread safe,
    // In concurrency scenario, add_rule_check_slot must be guarded by SlotChain.RWMutex#Lock
    pub fn add_rule_check_slot(&mut self, s: Arc<dyn RuleCheckSlot>) {
        self.rule_checks.push(s);
        self.rule_checks.sort_unstable_by_key(|a| a.order());
    }

    // add_stat_slot adds the StatSlot to the StatSlot list of the SlotChain.
    // All StatSlot in the list will be sorted according to StatSlot.order() in ascending order.
    // add_stat_slot is non-thread safe,
    // In concurrency scenario, add_stat_slot must be guarded by SlotChain.RWMutex#Lock
    pub fn add_stat_slot(&mut self, s: Arc<dyn StatSlot>) {
        self.stats.push(s);
        self.stats.sort_unstable_by_key(|a| a.order());
    }

    /// The entrance of slot chain
    /// Return the TokenResult
    pub fn entry(&self, ctx: ContextPtr) -> TokenResult {
        // execute prepare slot
        for s in &self.stat_pres {
            s.prepare(ctx.clone()); // Rc/Arc clone
        }

        // execute rule based checking slot
        cfg_if_async! {
            ctx.write().unwrap().reset_result_to_pass(),
            ctx.borrow_mut().reset_result_to_pass()
        };
        for s in &self.rule_checks {
            let res = s.check(&ctx);
            // check slot result
            if res.is_blocked() {
                cfg_if_async! {
                    ctx.write().unwrap().set_result(res.clone()),
                    ctx.borrow_mut().set_result(res.clone())
                };
            }
        }

        cfg_if_async! {
            let ctx_ptr = ctx.read().unwrap(),
            let ctx_ptr = ctx.borrow()
        };
        // execute statistic slot
        for s in &self.stats {
            // indicate the result of rule based checking slot.
            if ctx_ptr.result().is_pass() {
                s.on_entry_pass(ctx.clone()) // Rc/Arc clone
            } else {
                // The block error should not be nil.
                s.on_entry_blocked(ctx.clone(), ctx_ptr.result().block_err()) // Rc/Arc clone
            }
        }
        ctx_ptr.result().clone()
    }
}

#[cfg(test)]
pub(crate) use test::aggregation::{MockRuleCheckSlot, MockStatPrepareSlot, MockStatSlot};

#[cfg(test)]
mod test {
    use super::super::{
        BlockType, ConcurrencyStat, MetricItem, MetricItemRetriever, MockStatNode, NopReadStat,
        ReadStat, ResourceType, ResourceWrapper, ResultStatus, SentinelEntry, SentinelInput,
        TimePredicate, TrafficType, WriteStat,
    };
    use super::*;
    use crate::Result;
    use std::sync::Arc;

    // here we test three kinds of slots one by one
    mod single {
        use super::*;
        struct StatPrepareSlotMock {
            pub(self) name: String,
            pub(self) order: u32,
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
                    sc.add_stat_prepare_slot(Arc::new(StatPrepareSlotMock {
                        name: String::from(format!("mock{}", order)),
                        order,
                    }))
                }
            }
            assert_eq!(sc.stat_pres.len(), 50);
            for (i, s) in sc.stat_pres.into_iter().enumerate() {
                assert_eq!(
                    s.clone()
                        .as_any_arc()
                        .downcast::<StatPrepareSlotMock>()
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
                    sc.add_rule_check_slot(Arc::new(RuleCheckSlotMock {
                        name: String::from(format!("mock{}", order)),
                        order,
                    }))
                }
            }
            assert_eq!(sc.rule_checks.len(), 50);
            for (i, s) in sc.rule_checks.into_iter().enumerate() {
                assert_eq!(
                    s.clone()
                        .as_any_arc()
                        .downcast::<RuleCheckSlotMock>()
                        .unwrap()
                        .name,
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
                    sc.add_stat_slot(Arc::new(StatSlotMock {
                        name: String::from(format!("mock{}", order)),
                        order,
                    }))
                }
            }
            assert_eq!(sc.stats.len(), 50);
            for (i, s) in sc.stats.into_iter().enumerate() {
                assert_eq!(
                    s.clone()
                        .as_any_arc()
                        .downcast::<StatSlotMock>()
                        .unwrap()
                        .name,
                    format!("mock{}", i)
                );
            }
        }
    }

    pub(crate) mod aggregation {
        use super::*;
        use mockall::predicate::*;
        use mockall::*;

        // these signatures are necessary, don't remove them
        // because when use macro `mock!`, we have to supply the signatures expected to be mocked
        // otherwise, we cannot call `expect_xx()` on mocked objects
        /// MockStatPrepareSlot
        mock! {
            pub(crate) StatPrepareSlot {}
            impl BaseSlot for StatPrepareSlot {}
            impl StatPrepareSlot for StatPrepareSlot { fn prepare(&self, ctx: ContextPtr); }
        }

        /// MockRuleCheckSlot
        mock! {
            pub(crate) RuleCheckSlot {}
            impl BaseSlot for RuleCheckSlot {}
            impl RuleCheckSlot for RuleCheckSlot { fn check(&self, ctx: &ContextPtr) -> TokenResult; }
        }

        /// MockStatSlot
        mock! {
            pub(crate) StatSlot {}
            impl BaseSlot for StatSlot {}
            impl StatSlot for StatSlot {
                fn on_entry_pass(&self, ctx: ContextPtr);
                fn on_entry_blocked(&self, ctx: ContextPtr, block_error: Option<BlockError>);
                fn on_completed(&self, ctx: ContextPtr);
            }
        }

        #[test]
        fn pass_and_exit() {
            let mut ps = Arc::new(MockStatPrepareSlot::new());
            let mut rcs1 = Arc::new(MockRuleCheckSlot::new());
            let mut rcs2 = Arc::new(MockRuleCheckSlot::new());
            let mut ssm = Arc::new(MockStatSlot::new());
            let mut seq = Sequence::new();
            Arc::get_mut(&mut ps)
                .unwrap()
                .expect_prepare()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Arc::get_mut(&mut rcs1)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_pass());
            Arc::get_mut(&mut rcs2)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_pass());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_pass()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_blocked()
                .never()
                .return_const(());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_completed()
                .once()
                .in_sequence(&mut seq)
                .return_const(());

            let mut sc = SlotChain::new();
            sc.add_stat_prepare_slot(ps.clone());
            sc.add_rule_check_slot(rcs1.clone());
            sc.add_rule_check_slot(rcs2.clone());
            sc.add_stat_slot(ssm.clone());
            let sc = Arc::new(sc);

            let mut ctx = EntryContext::new();
            let rw = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
            ctx.set_resource(rw);
            ctx.set_stat_node(Arc::new(MockStatNode::new()));
            let ctx = Rc::new(RefCell::new(ctx));
            let entry = Rc::new(RefCell::new(SentinelEntry::new(ctx.clone(), sc.clone())));
            ctx.borrow_mut().set_entry(Rc::downgrade(&entry));

            let r = sc.entry(Rc::clone(&ctx));
            assert_eq!(ResultStatus::Pass, *r.status(), "should pass but blocked");
            sc.exit(Rc::clone(&ctx));
        }

        #[test]
        fn block() {
            let mut ps = Arc::new(MockStatPrepareSlot::new());
            let mut rcs1 = Arc::new(MockRuleCheckSlot::new());
            let mut rcs2 = Arc::new(MockRuleCheckSlot::new());
            let mut ssm = Arc::new(MockStatSlot::new());

            let mut seq = Sequence::new();
            Arc::get_mut(&mut ps)
                .unwrap()
                .expect_prepare()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Arc::get_mut(&mut rcs1)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_pass());
            Arc::get_mut(&mut rcs2)
                .unwrap()
                .expect_check()
                .once()
                .in_sequence(&mut seq)
                .returning(|_ctx| TokenResult::new_blocked(BlockType::Flow));
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_pass()
                .never()
                .return_const(());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_blocked()
                .once()
                .in_sequence(&mut seq)
                .return_const(());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_completed()
                .never()
                .return_const(());

            let mut sc = SlotChain::new();
            sc.add_stat_prepare_slot(ps);
            sc.add_rule_check_slot(rcs1);
            sc.add_rule_check_slot(rcs2);
            sc.add_stat_slot(ssm);
            let sc = Arc::new(sc);

            let mut ctx = EntryContext::new();
            let rw = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
            ctx.set_resource(rw);
            ctx.set_stat_node(Arc::new(MockStatNode::new()));
            let ctx = Rc::new(RefCell::new(ctx));
            let entry = Rc::new(RefCell::new(SentinelEntry::new(
                Rc::clone(&ctx),
                sc.clone(),
            )));
            ctx.borrow_mut().set_entry(Rc::downgrade(&entry));

            let r = sc.entry(Rc::clone(&ctx));
            assert_eq!(
                ResultStatus::Blocked,
                *r.status(),
                "should blocked but pass"
            );
            assert_eq!(
                BlockType::Flow,
                r.block_err().unwrap().block_type(),
                "should blocked by BlockType Flow"
            );
            sc.exit(Rc::clone(&ctx));
        }

        struct StatPrepareSlotBadMock {}

        impl BaseSlot for StatPrepareSlotBadMock {}

        impl StatPrepareSlot for StatPrepareSlotBadMock {
            fn prepare(&self, ctx: ContextPtr) {
                panic!("sentinel internal panic for test");
            }
        }
        #[test]
        #[should_panic(expected = "sentinel internal panic for test")]
        fn should_panic() {
            let ps = Arc::new(StatPrepareSlotBadMock {});
            let mut rcs1 = Arc::new(MockRuleCheckSlot::new());
            let mut rcs2 = Arc::new(MockRuleCheckSlot::new());
            let mut ssm = Arc::new(MockStatSlot::new());

            Arc::get_mut(&mut rcs1)
                .unwrap()
                .expect_check()
                .never()
                .returning(|_ctx| TokenResult::new_pass());
            Arc::get_mut(&mut rcs2)
                .unwrap()
                .expect_check()
                .never()
                .returning(|_ctx| TokenResult::new_blocked(BlockType::Flow));
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_pass()
                .never()
                .return_const(());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_entry_blocked()
                .never()
                .return_const(());
            Arc::get_mut(&mut ssm)
                .unwrap()
                .expect_on_completed()
                .never()
                .return_const(());

            let mut sc = SlotChain::new();
            sc.add_stat_prepare_slot(ps);
            sc.add_rule_check_slot(rcs1);
            sc.add_rule_check_slot(rcs2);
            sc.add_stat_slot(ssm);
            let sc = Arc::new(sc);

            let mut ctx = EntryContext::new();
            let rw = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
            ctx.set_resource(rw);
            ctx.set_stat_node(Arc::new(MockStatNode::new()));
            let ctx = Rc::new(RefCell::new(ctx));
            let entry = Rc::new(RefCell::new(SentinelEntry::new(
                Rc::clone(&ctx),
                sc.clone(),
            )));
            ctx.borrow_mut().set_entry(Rc::downgrade(&entry));

            let r = sc.entry(Rc::clone(&ctx));
        }
    }
}
