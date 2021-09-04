use super::inbound_node;
use crate::{
    base::{
        BaseSlot, BlockError, ContextPtr, EntryContext, MetricEvent, StatNode, StatSlot,
        TrafficType,
    },
    utils::curr_time_millis,
};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const STAT_SLOT_ORDER: u32 = 1000;

lazy_static! {
    pub static ref DEFAULT_RESOURCE_STAT_SLOT: Arc<ResourceNodeStatSlot> =
        Arc::new(ResourceNodeStatSlot {});
}

pub fn default_resource_stat_slot() -> Arc<ResourceNodeStatSlot> {
    DEFAULT_RESOURCE_STAT_SLOT.clone()
}

pub struct ResourceNodeStatSlot {}

impl ResourceNodeStatSlot {
    fn record_pass_for(&self, node: Arc<dyn StatNode>, count: u32) {
        node.increase_concurrency();
        node.add_count(MetricEvent::Pass, count as u64);
    }

    fn record_block_for(&self, node: Arc<dyn StatNode>, count: u32) {
        node.add_count(MetricEvent::Block, count as u64)
    }

    fn record_complete_for(&self, node: Arc<dyn StatNode>, count: u32, round_trip: u64) {
        // todo: cannot capture error now
        node.add_count(MetricEvent::Rt, round_trip as u64);
        node.add_count(MetricEvent::Complete, count as u64);
        node.decrease_concurrency();
    }
}

impl BaseSlot for ResourceNodeStatSlot {
    fn order(&self) -> u32 {
        STAT_SLOT_ORDER
    }
}

impl StatSlot for ResourceNodeStatSlot {
    fn on_entry_pass(&self, ctx: ContextPtr) {
        cfg_if_async! {
            let ctx = ctx.read().unwrap(),
            let ctx = ctx.borrow()
        };
        let res = ctx.resource();
        let input = ctx.input();
        if let Some(stat_node) = ctx.stat_node().clone() {
            self.record_pass_for(stat_node, input.batch_count());
            if *res.traffic_type() == TrafficType::Inbound {
                self.record_pass_for(inbound_node(), input.batch_count())
            }
        }
    }

    fn on_entry_blocked(&self, ctx: ContextPtr, block_error: Option<BlockError>) {
        cfg_if_async! {
            let ctx = ctx.read().unwrap(),
            let ctx = ctx.borrow()
        };
        let res = ctx.resource();
        let input = ctx.input();
        if let Some(stat_node) = ctx.stat_node().clone() {
            self.record_block_for(stat_node, input.batch_count());
            if *res.traffic_type() == TrafficType::Inbound {
                self.record_block_for(inbound_node(), input.batch_count())
            }
        }
    }

    cfg_async! {
        fn on_completed(&self, ctx: ContextPtr) {
            let mut round_trip = curr_time_millis() - ctx.read().unwrap().start_time();
            ctx.write().unwrap().set_round_trip(round_trip);
            if let Some(stat_node) = ctx.read().unwrap().stat_node().clone() {
                self.record_complete_for(stat_node, ctx.read().unwrap().input().batch_count(), round_trip);
                if *ctx.read().unwrap().resource().traffic_type() == TrafficType::Inbound {
                    self.record_block_for(inbound_node(), ctx.read().unwrap().input().batch_count());
                }
            }
        }
    }

    cfg_not_async! {
        fn on_completed(&self, ctx: ContextPtr) {
            let round_trip = curr_time_millis() - ctx.borrow().start_time();
            ctx.borrow_mut().set_round_trip(round_trip);
            if let Some(stat_node) = ctx.borrow().stat_node().clone() {
                self.record_complete_for(stat_node, ctx.borrow().input().batch_count(), round_trip);
                if *ctx.borrow().resource().traffic_type() == TrafficType::Inbound {
                    self.record_block_for(inbound_node(), ctx.borrow().input().batch_count());
                }
            }
        }
    }
}
