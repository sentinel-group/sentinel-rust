use super::*;
use crate::base::{BaseSlot, BlockType, EntryContext, RuleCheckSlot, TokenResult};
use lazy_static::lazy_static;
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
    fn check(&self, ctx: &mut EntryContext) -> TokenResult {
        let res = ctx.resource().name().clone();
        if res.is_empty() {
            return ctx.result().clone();
        }
        if can_pass_check(ctx, &res).is_some() {
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
fn can_pass_check(ctx: &EntryContext, res: &String) -> Option<Arc<Rule>> {
    let breakers = get_breakers_of_resource(res);
    for breaker in breakers {
        if !breaker.try_pass(ctx) {
            return Some(Arc::clone(breaker.bound_rule()));
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::{EntryContext, ResourceType, ResourceWrapper, TrafficType};

    #[test]
    #[ignore]
    fn check_blocked() {
        let rules = vec![Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::Custom(101),
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        })];
        set_circuit_breaker_generator(
            BreakerStrategy::Custom(101),
            Box::new(
                move |_: Arc<Rule>,
                      _: Option<Arc<CounterLeapArray>>|
                      -> Arc<dyn CircuitBreakerTrait> {
                    let mut breaker = MockCircuitBreaker::new();
                    let rule = Rule {
                        resource: "abc".into(),
                        strategy: BreakerStrategy::Custom(101),
                        retry_timeout_ms: 3000,
                        min_request_amount: 10,
                        stat_interval_ms: 10000,
                        max_allowed_rt_ms: 50,
                        threshold: 0.5,
                        ..Default::default()
                    };
                    breaker.expect_try_pass().return_const(false);
                    breaker.expect_bound_rule().return_const(Arc::new(rule));
                    Arc::new(breaker)
                },
            ),
        )
        .unwrap();
        load_rules(rules);
        let res_name = String::from("abc");
        assert_eq!(get_breakers_of_resource(&res_name).len(), 1);

        let slot = Slot {};
        let mut ctx = EntryContext::new();
        let res = ResourceWrapper::new(res_name, ResourceType::Common, TrafficType::Inbound);
        ctx.set_resource(res);
        let token = slot.check(&mut ctx);
        assert!(token.is_blocked());
        clear_rules();
    }

    #[test]
    #[ignore]
    fn check_pass() {
        let rules = vec![Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::Custom(101),
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        })];
        set_circuit_breaker_generator(
            BreakerStrategy::Custom(101),
            Box::new(
                move |_: Arc<Rule>,
                      _: Option<Arc<CounterLeapArray>>|
                      -> Arc<dyn CircuitBreakerTrait> {
                    let mut breaker = MockCircuitBreaker::new();
                    breaker.expect_try_pass().return_const(true);
                    Arc::new(breaker)
                },
            ),
        )
        .unwrap();
        load_rules(rules);
        let res_name = String::from("abc");
        assert_eq!(get_breakers_of_resource(&res_name).len(), 1);

        let slot = Slot {};
        let mut ctx = EntryContext::new();
        let res = ResourceWrapper::new(res_name, ResourceType::Common, TrafficType::Inbound);
        ctx.set_resource(res);
        let token = slot.check(&mut ctx);
        assert!(token.is_pass());
        assert!(ctx.result().is_pass());
        clear_rules();
    }
}
