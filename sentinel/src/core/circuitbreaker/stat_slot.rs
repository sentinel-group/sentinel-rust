use super::*;
use crate::base::{BaseSlot, BlockError, EntryContext, MetricEvent, StatSlot};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const STAT_SLOT_ORDER: u32 = 5000;

/// MetricStatSlot records metrics for circuit breaker on invocation completed.
/// MetricStatSlot must be filled into slot chain if circuit breaker is alive.
pub struct MetricStatSlot {}

lazy_static! {
    pub static ref DEFAULT_METRIC_STAT_SLOT: Arc<MetricStatSlot> = Arc::new(MetricStatSlot {});
}

pub fn default_metric_stat_slot() -> Arc<MetricStatSlot> {
    DEFAULT_METRIC_STAT_SLOT.clone()
}

impl BaseSlot for MetricStatSlot {
    fn order(&self) -> u32 {
        STAT_SLOT_ORDER
    }
}

impl StatSlot for MetricStatSlot {
    fn on_entry_pass(&self, _ctx: Rc<RefCell<EntryContext>>) {}

    fn on_entry_blocked(&self, _ctx: Rc<RefCell<EntryContext>>, _block_error: Option<BlockError>) {}

    fn on_completed(&self, ctx: Rc<RefCell<EntryContext>>) {
        let ctx = ctx.borrow();
        let res = ctx.resource().name();
        let rt = ctx.round_trip();
        for cb in get_breakers_of_resource(res) {
            cb.on_request_complete(rt, ctx.get_err());
        }
    }
}
