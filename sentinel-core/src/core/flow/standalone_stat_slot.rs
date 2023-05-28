use super::*;
use crate::base::{BaseSlot, BlockError, EntryContext, MetricEvent, StatSlot};
use lazy_static::lazy_static;
use std::sync::Arc;

const STAT_SLOT_ORDER: u32 = 3000;

pub struct StandaloneStatSlot {}

lazy_static! {
    pub static ref DEFAULT_STAND_ALONE_STAT_SLOT: Arc<StandaloneStatSlot> =
        Arc::new(StandaloneStatSlot {});
}

pub fn default_stand_alone_stat_slot() -> Arc<StandaloneStatSlot> {
    DEFAULT_STAND_ALONE_STAT_SLOT.clone()
}

impl BaseSlot for StandaloneStatSlot {
    fn order(&self) -> u32 {
        STAT_SLOT_ORDER
    }
}

impl StatSlot for StandaloneStatSlot {
    fn on_entry_pass(&self, ctx: &EntryContext) {
        let res = ctx.resource().name();
        let input = ctx.input();
        let tcs = get_traffic_controller_list_for(res);
        for tc in tcs {
            if !tc.stat().reuse_global() {
                tc.stat()
                    .write_only_metric()
                    .unwrap()
                    .add_count(MetricEvent::Pass, input.batch_count() as u64);
            }
        }
    }

    fn on_entry_blocked(&self, _ctx: &EntryContext, _block_error: BlockError) {}

    fn on_completed(&self, _ctx: &mut EntryContext) {}
}
