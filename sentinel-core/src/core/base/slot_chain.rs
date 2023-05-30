use super::{BlockError, ContextPtr, EntryContext, TokenResult, SLOT_INIT};
use crate::logging;
use crate::utils::AsAny;
use std::any::Any;
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
    /// prepare function do some initialization
    /// Such as: init statistic structure、node and etc
    /// The result of preparing would store in EntryContext
    /// All StatPrepareSlots execute in sequence
    /// prepare function should not throw panic.
    fn prepare(&self, _ctx: &mut EntryContext) {}
}

/// RuleCheckSlot is rule based checking strategy
/// All checking rule must implement this interface.
pub trait RuleCheckSlot: BaseSlot {
    // check function do some validation
    // It can break off the slot pipeline
    // Each TokenResult will return check result
    // The upper logic will control pipeline according to SlotResult.
    fn check(&self, ctx: &mut EntryContext) -> TokenResult {
        ctx.result().clone()
    }
}

/// StatSlot is responsible for counting all custom biz metrics.
/// StatSlot would not handle any panic, and pass up all panic to slot chain
pub trait StatSlot: BaseSlot {
    /// OnEntryPass function will be invoked when StatPrepareSlots and RuleCheckSlots execute pass
    /// StatSlots will do some statistic logic, such as QPS、log、etc
    fn on_entry_pass(&self, _ctx: &EntryContext) {}
    /// on_entry_blocked function will be invoked when StatPrepareSlots and RuleCheckSlots fail to execute
    /// It may be inbound flow control or outbound cir
    /// StatSlots will do some statistic logic, such as QPS、log、etc
    /// blockError introduce the block detail
    fn on_entry_blocked(&self, _ctx: &EntryContext, _block_error: BlockError) {}
    /// on_completed function will be invoked when chain exits.
    /// The semantics of on_completed is the entry passed and completed
    /// Note: blocked entry will not call this function
    fn on_completed(&self, _ctx: &mut EntryContext) {}
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

impl Default for SlotChain {
    fn default() -> Self {
        Self {
            stat_pres: Vec::with_capacity(SLOT_INIT),
            rule_checks: Vec::with_capacity(SLOT_INIT),
            stats: Vec::with_capacity(SLOT_INIT),
        }
    }
}

impl SlotChain {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn exit(&self, ctx_ptr: ContextPtr) {
        let mut ctx = ctx_ptr.write().unwrap();
        if ctx.entry().is_none() {
            logging::error!("SentinelEntry is nil in SlotChain.exit()");
            return;
        }
        if ctx.is_blocked() {
            return;
        }
        // The on_completed is called only when entry passed
        for s in &self.stats {
            s.on_completed(&mut ctx);
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
    pub fn entry(&self, ctx_ptr: ContextPtr) -> TokenResult {
        let mut ctx = ctx_ptr.write().unwrap();
        // execute prepare slot
        for s in &self.stat_pres {
            s.prepare(&mut ctx); // Rc/Arc clone
        }

        // execute rule based checking slot
        ctx.reset_result_to_pass();
        for s in &self.rule_checks {
            let res = s.check(&mut ctx);
            // check slot result
            if res.is_blocked() {
                ctx.set_result(res.clone());
            }
        }

        // execute statistic slot
        for s in &self.stats {
            // indicate the result of rule based checking slot.
            if ctx.result().is_pass() {
                s.on_entry_pass(&ctx) // Rc/Arc clone
            } else if ctx.result().is_blocked() {
                // The block error should not be none.
                s.on_entry_blocked(&ctx, ctx.result().block_err().unwrap()) // Rc/Arc clone
            }
        }
        ctx.result().clone()
    }
}

#[cfg(test)]
pub(crate) use test::aggregation::{MockRuleCheckSlot, MockStatPrepareSlot, MockStatSlot};

#[cfg(test)]
mod test {
    use super::super::{
        BlockType, EntryContext, MockStatNode, ResourceType, ResourceWrapper, SentinelEntry,
        TrafficType,
    };
    use super::*;
    use std::sync::RwLock;

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
                        name: format!("mock{}", order),
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
                        name: format!("mock{}", order),
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
                        name: format!("mock{}", order),
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
        mock! {
            pub(crate) StatPrepareSlot {}
            impl BaseSlot for StatPrepareSlot {}
            impl StatPrepareSlot for StatPrepareSlot { fn prepare(&self, ctx: &mut EntryContext); }
        }

        mock! {
            pub(crate) RuleCheckSlot {}
            impl BaseSlot for RuleCheckSlot {}
            impl RuleCheckSlot for RuleCheckSlot { fn check(&self, ctx: &mut EntryContext) -> TokenResult; }
        }

        mock! {
            pub(crate) StatSlot {}
            impl BaseSlot for StatSlot {}
            impl StatSlot for StatSlot {
                fn on_entry_pass(&self, ctx: &EntryContext);
                fn on_entry_blocked(&self, ctx: &EntryContext, block_error: BlockError);
                fn on_completed(&self, ctx: &mut EntryContext);
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
            let ctx = Arc::new(RwLock::new(ctx));
            let entry = Arc::new(RwLock::new(SentinelEntry::new(ctx.clone(), sc.clone())));
            ctx.write().unwrap().set_entry(Arc::downgrade(&entry));

            let r = sc.entry(Arc::clone(&ctx));
            assert!(r.is_pass(), "should pass but blocked");
            sc.exit(Arc::clone(&ctx));
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
            let ctx = Arc::new(RwLock::new(ctx));
            let entry = Arc::new(RwLock::new(SentinelEntry::new(
                Arc::clone(&ctx),
                sc.clone(),
            )));
            ctx.write().unwrap().set_entry(Arc::downgrade(&entry));

            let r = sc.entry(Arc::clone(&ctx));
            assert!(r.is_blocked(), "should blocked but pass");
            assert_eq!(
                BlockType::Flow,
                r.block_err().unwrap().block_type(),
                "should blocked by BlockType Flow"
            );
            sc.exit(Arc::clone(&ctx));
        }

        struct StatPrepareSlotBadMock {}

        impl BaseSlot for StatPrepareSlotBadMock {}

        impl StatPrepareSlot for StatPrepareSlotBadMock {
            fn prepare(&self, _ctx: &mut EntryContext) {
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
            let ctx = Arc::new(RwLock::new(ctx));
            let entry = Arc::new(RwLock::new(SentinelEntry::new(
                Arc::clone(&ctx),
                sc.clone(),
            )));
            ctx.write().unwrap().set_entry(Arc::downgrade(&entry));

            sc.entry(Arc::clone(&ctx));
        }
    }
}
