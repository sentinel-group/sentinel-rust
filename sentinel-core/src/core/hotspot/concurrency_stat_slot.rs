use super::*;
use crate::{
    base::{BaseSlot, BlockError, EntryContext, StatSlot},
    logging,
};
use lazy_static::lazy_static;
use std::sync::{atomic::Ordering, Arc};

const STAT_SLOT_ORDER: u32 = 4000;

/// ConcurrencyStatSlot is to record the Concurrency statistic for all arguments
pub struct ConcurrencyStatSlot {}

lazy_static! {
    pub static ref DEFAULT_STAND_ALONE_STAT_SLOT: Arc<ConcurrencyStatSlot> =
        Arc::new(ConcurrencyStatSlot {});
}

pub fn default_stand_alone_stat_slot() -> Arc<ConcurrencyStatSlot> {
    DEFAULT_STAND_ALONE_STAT_SLOT.clone()
}

impl BaseSlot for ConcurrencyStatSlot {
    fn order(&self) -> u32 {
        STAT_SLOT_ORDER
    }
}

impl StatSlot for ConcurrencyStatSlot {
    fn on_entry_pass(&self, ctx: &EntryContext) {
        let res = ctx.resource().name();
        let tcs = get_traffic_controller_list_for(res);
        for tc in tcs {
            if tc.rule().metric_type != MetricType::Concurrency {
                continue;
            }
            if let Some(arg) = tc.extract_args(ctx) {
                let metric = tc.metric();
                match metric.concurrency_counter.get(&arg) {
                    Some(counter) => {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                    None => {
                        logging::debug!("[ConcurrencyStatSlot on_entry_passed] Parameter does not exist in ConcurrencyCounter., argument: {:?}", arg);
                    }
                }
            }
        }
    }

    fn on_entry_blocked(&self, _ctx: &EntryContext, _block_error: BlockError) {}

    fn on_completed(&self, ctx: &mut EntryContext) {
        let res = ctx.resource().name();
        let tcs = get_traffic_controller_list_for(res);
        for tc in tcs {
            if tc.rule().metric_type != MetricType::Concurrency {
                continue;
            }
            if let Some(arg) = tc.extract_args(ctx) {
                let metric = tc.metric();
                match metric.concurrency_counter.get(&arg) {
                    Some(counter) => {
                        counter.fetch_sub(1, Ordering::SeqCst);
                    }
                    None => {
                        logging::debug!("[ConcurrencyStatSlot on_entry_passed] Parameter does not exist in ConcurrencyCounter., argument: {:?}", arg);
                    }
                }
            }
        }
    }
}
