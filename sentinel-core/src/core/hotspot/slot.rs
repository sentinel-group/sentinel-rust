use super::*;
use crate::{
    base::{BaseSlot, ContextPtr, RuleCheckSlot, TokenResult},
    utils,
};
use lazy_static::lazy_static;
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
    fn check(&self, ctx_ptr: &ContextPtr) -> TokenResult {
        cfg_if_async! {
            let mut ctx = ctx_ptr.write().unwrap(),
            let mut ctx = ctx_ptr.borrow_mut()
        };
        let res = ctx.resource().name();
        let batch = ctx.input().batch_count();
        let tcs = get_traffic_controller_list_for(res);
        for tc in tcs {
            let extracted = tc.extract_args(&ctx_ptr);
            if let Some(arg) = extracted {
                let r = tc.perform_checking(arg, batch);
                match r {
                    TokenResult::Pass => {}
                    TokenResult::Blocked(_) => {
                        ctx.set_result(r);
                        return ctx.result().clone();
                    }
                    TokenResult::Wait(nanos_to_wait) => {
                        utils::sleep_for_ns(nanos_to_wait);
                    }
                }
            }
        }
        return ctx.result().clone();
    }
}
