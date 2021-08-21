use super::*;
use crate::{
    base::{
        BaseSlot, BlockType, EntryContext, MetricEvent, ResultStatus, RuleCheckSlot, StatNode,
        StatSlot, TokenResult,
    },
    logging, stat, utils,
    utils::AsAny,
};
use lazy_static::lazy_static;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const RULE_CHECK_SLOT_ORDER: u32 = 5000;

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
        if res.len() == 0 {
            return ctx.result().clone();
        }
        if let Some(rule) = can_pass_check(ctx_cloned) {
            ctx.set_result(TokenResult::new_blocked_with_msg(
                BlockType::CircuitBreaking,
                "circuit breaker check blocked".into(),
            ));
        }
        return ctx.result().clone();
    }
}

/// `None` indicates it passes
/// `Some(rule)` indicates it is broke by the rule
fn can_pass_check(ctx: Rc<RefCell<EntryContext>>) -> Option<Arc<Rule>> {
    let breakers = get_breakers_of_resource(ctx.borrow().resource().name());
    for breaker in breakers {
        if !breaker.try_pass(Rc::clone(&ctx)) {
            return Some(Arc::clone(breaker.bound_rule()));
        }
    }
    return None;
}

#[cfg(test)]
mod test {
    use super::*;
}
