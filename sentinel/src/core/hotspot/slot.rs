use super::*;
use crate::{
    base::{
        BaseSlot, EntryContext, MetricEvent, ResultStatus, RuleCheckSlot, StatNode, StatSlot,
        TokenResult,
    },
    logging, stat, utils,
    utils::AsAny,
};
use lazy_static::lazy_static;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const RULE_CHECK_SLOT_ORDER: u32 = 4000;

/// A RuleSlot for flow related metrics
pub struct Slot {}

lazy_static! {
    pub static ref DEFAULT_SLOT: Arc<Slot> = Arc::new(Slot {});
}

pub fn default_slot() -> Arc<Slot> {
    DEFAULT_SLOT.clone()
}

impl BaseSlot for Slot {
    fn order(&self) -> u32 {
        RULE_CHECK_SLOT_ORDER
    }
}

impl RuleCheckSlot for Slot {
    fn check(&self, ctx: &Rc<RefCell<EntryContext>>) -> TokenResult {
        let ctx_cloned = Rc::clone(&ctx);
        let mut ctx = ctx.borrow_mut();
        let res = ctx.resource().name();
        let input = ctx.input();
        let batch = input.batch_count();

        let tcs = get_traffic_controller_list_for(res);
        for tc in tcs {
            if let Some(arg) = tc.extract_args(&ctx_cloned) {
                let r = tc.perform_checking(arg, batch);
                match r.status() {
                    ResultStatus::Pass => {}
                    ResultStatus::Blocked => {
                        ctx.set_result(r);
                        return ctx.result().clone();
                    }
                    ResultStatus::ShouldWait => {
                        let nanos_to_wait = r.nanos_to_wait();
                        utils::sleep_for_ns(nanos_to_wait);
                    }
                }
            }
        }
        ctx.result().clone()
    }
}
