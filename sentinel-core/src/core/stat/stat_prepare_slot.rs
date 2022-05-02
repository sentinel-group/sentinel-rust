use super::get_or_create_resource_node;
use crate::base::{BaseSlot, ContextPtr, StatPrepareSlot};
use lazy_static::lazy_static;
use std::sync::Arc;

const PREPARE_SLOT_ORDER: u32 = 1000;

lazy_static! {
    pub static ref DEFAULT_RESOURCE_NODE_PREPARE_SLOT: Arc<ResourceNodePrepareSlot> =
        Arc::new(ResourceNodePrepareSlot {});
}

pub fn default_resource_node_prepare_slot() -> Arc<ResourceNodePrepareSlot> {
    DEFAULT_RESOURCE_NODE_PREPARE_SLOT.clone()
}

pub struct ResourceNodePrepareSlot {}

impl BaseSlot for ResourceNodePrepareSlot {
    fn order(&self) -> u32 {
        PREPARE_SLOT_ORDER
    }
}

impl StatPrepareSlot for ResourceNodePrepareSlot {
    fn prepare(&self, ctx_ptr: ContextPtr) {
        cfg_if_async! {
            let mut ctx = ctx_ptr.write().unwrap(),
            let mut ctx = ctx_ptr.borrow_mut()
        };
        let node =
            get_or_create_resource_node(ctx.resource().name(), ctx.resource().resource_type());
        ctx.set_stat_node(node);
    }
}
