use super::global_slot_chain;
use crate::base::{
    EntryContext, EntryStrongPtr, ParamsList, ParamsMap, ResourceType, ResourceWrapper,
    SentinelEntry, SentinelInput, SlotChain, TokenResult, TrafficType,
};
use crate::utils::format_time_nanos_curr;
use crate::{Error, Result};
use std::sync::Arc;
use std::sync::RwLock;

// EntryBuilder is the basic API of Sentinel.
pub struct EntryBuilder {
    resource_name: String,
    resource_type: ResourceType,
    traffic_type: TrafficType,
    batch_count: u32,
    flag: i32,
    slot_chain: Arc<SlotChain>,
    args: Option<ParamsList>,
    attachments: Option<ParamsMap>,
}

// or set all items in builder to None by default?
// then use builder to change `Default` ctx and initialize Entry seems better
impl Default for EntryBuilder {
    fn default() -> Self {
        EntryBuilder {
            resource_name: format_time_nanos_curr(),
            resource_type: ResourceType::default(),
            traffic_type: TrafficType::default(),
            batch_count: 1,
            flag: 0,
            slot_chain: global_slot_chain(),
            args: None,
            attachments: None,
        }
    }
}

impl EntryBuilder {
    pub fn new(resource_name: String) -> Self {
        EntryBuilder {
            resource_name,
            ..EntryBuilder::default()
        }
    }

    /// `build()` would consume EntryBuilder
    pub fn build(self) -> Result<EntryStrongPtr> {
        // get context from pool.
        let mut ctx = EntryContext::new();

        ctx.set_resource(ResourceWrapper::new(
            self.resource_name,
            self.resource_type,
            self.traffic_type,
        ));

        let mut input = SentinelInput::new(self.batch_count, self.flag);
        if let Some(args) = self.args {
            input.set_args(args);
        }
        if let Some(attachments) = self.attachments {
            input.set_attachments(attachments);
        }
        ctx.set_input(input);

        let ctx = Arc::new(RwLock::new(ctx));
        let entry = Arc::new(RwLock::new(SentinelEntry::new(
            Arc::clone(&ctx),
            Arc::clone(&self.slot_chain),
        )));
        ctx.write().unwrap().set_entry(Arc::downgrade(&entry));

        let r = self.slot_chain.entry(Arc::clone(&ctx));
        match r {
            TokenResult::Blocked(_) => {
                // todo:
                // if return block_error,
                // must deep copy the error, since Arc only clone pointer
                entry.read().unwrap().exit();
                Err(Error::msg(r.to_string()))
            }
            _ => Ok(EntryStrongPtr::new(entry)),
        }
    }

    pub fn with_resource_type(mut self, resource_type: ResourceType) -> Self {
        self.resource_type = resource_type;
        self
    }

    pub fn with_traffic_type(mut self, traffic_type: TrafficType) -> Self {
        self.traffic_type = traffic_type;
        self
    }

    pub fn with_batch_count(mut self, batch_count: u32) -> Self {
        self.batch_count = batch_count;
        self
    }

    pub fn with_flag(mut self, flag: i32) -> Self {
        self.flag = flag;
        self
    }

    pub fn with_slot_chain(mut self, slot_chain: Arc<SlotChain>) -> Self {
        self.slot_chain = slot_chain;
        self
    }

    pub fn with_args(mut self, args: Option<ParamsList>) -> Self {
        self.args = args;
        self
    }

    pub fn with_attachments(mut self, attachments: Option<ParamsMap>) -> Self {
        self.attachments = attachments;
        self
    }
}

pub fn trace_error(entry: &EntryStrongPtr, err: Error) {
    entry.set_err(err);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::{
        BlockType, MockRuleCheckSlot, MockStatNode, MockStatPrepareSlot, MockStatSlot, TokenResult,
    };
    use mockall::*;

    #[test]
    fn pass() {
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

        let builder = EntryBuilder::new("abc".into()).with_slot_chain(sc);
        let entry = builder.build().unwrap();
        assert_eq!("abc", entry.context().read().unwrap().resource().name());
        entry.exit();
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

        let builder = EntryBuilder::new("abc".into()).with_slot_chain(sc);
        assert!(builder.build().is_err());
    }
}
