use super::*;
use crate::{
    base::{
        BaseSlot, BlockType, ConcurrencyStat, ContextPtr, EntryContext, MetricEvent, ReadStat,
        ResultStatus, RuleCheckSlot, SentinelRule, Snapshot, StatNode, StatSlot, TokenResult,
        TrafficType,
    },
    logging, stat, system_metric, utils,
};
use lazy_static::lazy_static;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const RULE_CHECK_SLOT_ORDER: u32 = 3000;

/// A RuleSlot for flow related metrics
pub struct AdaptiveSlot {}

lazy_static! {
    pub static ref DEFAULT_ADAPTIVE_SLOT: Arc<AdaptiveSlot> = Arc::new(AdaptiveSlot {});
}

pub fn default_slot() -> Arc<AdaptiveSlot> {
    DEFAULT_ADAPTIVE_SLOT.clone()
}

impl BaseSlot for AdaptiveSlot {
    fn order(&self) -> u32 {
        RULE_CHECK_SLOT_ORDER
    }
}

impl RuleCheckSlot for AdaptiveSlot {
    cfg_async! {
        fn check(&self, ctx: &ContextPtr) -> TokenResult {
            let res_name = ctx.read().unwrap().resource().name().clone();
            if res_name.len() == 0 {
                return ctx.read().unwrap().result().clone();
            }
            let (passed, rule, snapshot) = can_pass_check(ctx, &res_name);
            if !passed {
                // never panic
                ctx.write().unwrap()
                    .set_result(TokenResult::new_blocked_with_cause(
                        BlockType::SystemFlow,
                        "concurrency exceeds threshold".into(),
                        rule.unwrap(),
                        snapshot.unwrap(),
                    ));
            }
            return ctx.read().unwrap().result().clone();
        }
    }

    cfg_not_async! {
        fn check(&self, ctx: &ContextPtr) -> TokenResult {
            let res_name = ctx.borrow().resource().name().clone();
            if res_name.len() == 0 {
                return ctx.borrow().result().clone();
            }
            let (passed, rule, snapshot) = can_pass_check(ctx, &res_name);
            if !passed {
                // never panic
                ctx.borrow_mut()
                    .set_result(TokenResult::new_blocked_with_cause(
                        BlockType::SystemFlow,
                        "concurrency exceeds threshold".into(),
                        rule.unwrap(),
                        snapshot.unwrap(),
                    ));
            }
            return ctx.borrow().result().clone();
        }
    }
}

fn can_pass_check(
    ctx: &ContextPtr,
    res: &String,
) -> (bool, Option<Arc<Rule>>, Option<Arc<Snapshot>>) {
    cfg_if_async! {
        let ctx = ctx.read().unwrap(),
        let ctx = ctx.borrow()
    };
    let stat_node = ctx.stat_node().unwrap();
    let batch_count = ctx.input().batch_count();
    for rule in get_rules_of_resource(res) {
        let threshold = rule.threshold;
        if rule.metric_type == MetricType::Concurrency {
            let curr_count = stat_node.current_concurrency();
            // if pass `batch_count` tasks in the `ctx`, the limits on concurrency would break
            if curr_count + batch_count > threshold {
                return (false, Some(rule), Some(Arc::new(curr_count)));
            }
        }
    }
    return (true, None, None);
}
