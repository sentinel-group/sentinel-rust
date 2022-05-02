use super::*;
use crate::{
    base::{BaseSlot, ContextPtr, RuleCheckSlot, StatNode, TokenResult},
    logging, stat, utils,
    utils::AsAny,
};
use lazy_static::lazy_static;
use std::sync::Arc;

const RULE_CHECK_SLOT_ORDER: u32 = 2000;

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
        let stat_node = ctx.stat_node();
        let input = ctx.input();
        let tcs = get_traffic_controller_list_for(res);
        for tc in tcs {
            let r = can_pass_check(tc, stat_node.clone(), input.batch_count());
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
        return ctx.result().clone();
    }
}

fn can_pass_check(
    tc: Arc<Controller>,
    given_node: Option<Arc<dyn StatNode>>,
    batch_count: u32,
) -> TokenResult {
    let actual_node = {
        match tc.rule().relation_strategy {
            RelationStrategy::Associated => {
                let node = stat::get_resource_node(&tc.rule().ref_resource).unwrap();
                let node = node.as_any_arc();
                let node = node.downcast_ref::<Arc<dyn StatNode>>().unwrap();
                Some(node.clone())
            }
            _ => given_node,
        }
    };
    match actual_node {
        Some(node) => tc.perform_checking(node, batch_count, 0),
        None => {
            logging::FREQUENT_ERROR_ONCE.call_once(|| {
                logging::error!(
                    "None statistics node for flow rule in FlowSlot.can_pass_check() {:?}",
                    tc.rule()
                );
            });
            TokenResult::new_pass()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::{
        EntryContext, MetricEvent, ResourceType, ResourceWrapper, SentinelInput, StatSlot,
        TrafficType,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn rule_check_slot() {
        let slot = Slot {};
        let stat_slot = StandaloneStatSlot {};
        let res_name = String::from("abc");
        let res =
            ResourceWrapper::new(res_name.clone(), ResourceType::Common, TrafficType::Inbound);
        let res_node = stat::get_or_create_resource_node(&res_name, &ResourceType::Common);
        let mut ctx = EntryContext::new();
        ctx.set_input(SentinelInput::new(1, 0));
        ctx.set_stat_node(res_node);
        ctx.set_resource(res);
        let ctx = Rc::new(RefCell::new(ctx));

        slot.check(&ctx);

        let r1 = Arc::new(Rule {
            resource: res_name.clone(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            // Use standalone statistic, using single-bucket-sliding-windows
            stat_interval_ms: 20000,
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            ..Default::default()
        });
        load_rules(vec![r1]);

        for _ in 0..50 {
            slot.check(&ctx);
            stat_slot.on_entry_pass(Rc::clone(&ctx));
        }
        assert_eq!(
            get_traffic_controller_list_for(&res_name)[0]
                .stat()
                .read_only_metric()
                .sum(MetricEvent::Pass),
            50
        );
    }
}
