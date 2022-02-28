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
    cfg_async! {
        fn prepare(&self, ctx: ContextPtr) {
            let node = get_or_create_resource_node(
                ctx.read().unwrap().resource().name(),
                ctx.read().unwrap().resource().resource_type(),
            );
            ctx.write().unwrap().set_stat_node(node);
        }
    }

    cfg_not_async! {
        fn prepare(&self, ctx: ContextPtr) {
            let node = get_or_create_resource_node(
                ctx.borrow().resource().name(),
                ctx.borrow().resource().resource_type(),
            );
            ctx.borrow_mut().set_stat_node(node);
        }
    }
}
