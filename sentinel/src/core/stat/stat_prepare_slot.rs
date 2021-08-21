use super::get_or_create_resource_node;
use crate::base::{BaseSlot, EntryContext, StatPrepareSlot};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::rc::Rc;
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
    fn prepare(&self, ctx: Rc<RefCell<EntryContext>>) {
        let node = get_or_create_resource_node(
            ctx.borrow().resource().name(),
            ctx.borrow().resource().resource_type(),
        );
        ctx.borrow_mut().set_stat_node(node);
    }
}
