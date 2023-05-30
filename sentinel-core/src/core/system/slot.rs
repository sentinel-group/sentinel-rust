use super::*;
use crate::{
    base::{
        BaseSlot, BlockType, ConcurrencyStat, EntryContext, MetricEvent, ReadStat, RuleCheckSlot,
        Snapshot, TokenResult, TrafficType,
    },
    stat, system_metric,
};
use lazy_static::lazy_static;
use std::sync::Arc;

const RULE_CHECK_SLOT_ORDER: u32 = 1000;

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
    fn check(&self, ctx: &mut EntryContext) -> TokenResult {
        let res = ctx.resource();
        let traffic_type = res.traffic_type();
        if *traffic_type == TrafficType::Outbound {
            return ctx.result().clone();
        }
        let rules = get_rules();
        for rule in rules {
            let (passed, msg, snapshot) = can_pass_check(&rule);
            if passed {
                continue;
            }
            // never panic
            ctx.set_result(TokenResult::new_blocked_with_cause(
                BlockType::SystemFlow,
                msg,
                rule.clone(),
                snapshot.unwrap(),
            ));
            return ctx.result().clone();
        }
        return ctx.result().clone();
    }
}

fn can_pass_check(rule: &Arc<Rule>) -> (bool, String, Option<Arc<Snapshot>>) {
    let threshold = rule.threshold;
    let mut res = true;
    let mut msg = String::new();
    let mut snapshot = None;
    match rule.metric_type {
        MetricType::InboundQPS => {
            let qps = stat::inbound_node().qps(MetricEvent::Pass);
            res = qps < threshold;
            if !res {
                msg = "system qps check blocked".into();
                snapshot = Some(Arc::new(qps) as Arc<Snapshot>);
            }
        }
        MetricType::Concurrency => {
            let n = stat::inbound_node().current_concurrency() as f64;
            res = n < threshold;
            if !res {
                msg = "system concurrency check blocked".into();
                snapshot = Some(Arc::new(n) as Arc<Snapshot>);
            }
        }
        MetricType::AvgRT => {
            let rt = stat::inbound_node().avg_rt();
            res = rt < threshold;
            if !res {
                msg = "system avg rt check blocked".into();
                snapshot = Some(Arc::new(rt) as Arc<Snapshot>);
            }
        }
        MetricType::Load => {
            let l = system_metric::current_load();
            if l > threshold && (rule.strategy != AdaptiveStrategy::BBR || !check_bbr_simple()) {
                res = false;
                msg = "system load check blocked".into();
            }
            snapshot = Some(Arc::new(l) as Arc<Snapshot>);
        }
        MetricType::CpuUsage => {
            let c = system_metric::current_cpu_usage() as f64;
            if c > threshold && (rule.strategy != AdaptiveStrategy::BBR || !check_bbr_simple()) {
                res = false;
                msg = "system cpu usage check blocked".into();
            }
            snapshot = Some(Arc::new(c) as Arc<Snapshot>);
        }
    }
    (res, msg, snapshot)
}

fn check_bbr_simple() -> bool {
    let global_inbound = &stat::inbound_node();
    let concurrency = global_inbound.current_concurrency() as f64;
    let min_rt = global_inbound.min_rt();
    let max_complete = global_inbound.max_avg(MetricEvent::Complete);
    !(concurrency > 1.0 && concurrency > max_complete * min_rt / 1000.0)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::{EntryContext, ResourceType, ResourceWrapper, SentinelInput};

    #[test]
    fn unsuitable_traffic_type() {
        let slot = AdaptiveSlot {};
        let res_name = String::from("test");
        let res_node = stat::get_or_create_resource_node(&res_name, &ResourceType::Common);
        let rw = ResourceWrapper::new(res_name, ResourceType::Common, TrafficType::Outbound);
        let mut ctx = EntryContext::new();
        ctx.set_input(SentinelInput::new(1, 0));
        ctx.set_stat_node(res_node);
        ctx.set_resource(rw);
        let r = slot.check(&mut ctx);
        assert_eq!(r, *ctx.result());
    }

    #[test]
    fn empty_rule() {
        let slot = AdaptiveSlot {};
        let res_name = String::from("test");
        let res_node = stat::get_or_create_resource_node(&res_name, &ResourceType::Common);
        let rw = ResourceWrapper::new(res_name, ResourceType::Common, TrafficType::Outbound);
        let mut ctx = EntryContext::new();
        ctx.set_input(SentinelInput::new(1, 0));
        ctx.set_stat_node(res_node);
        ctx.set_resource(rw);
        let r = slot.check(&mut ctx);
        assert!(r.is_pass());
    }

    #[test]
    #[ignore]
    fn valid_concurrency() {
        let rule = Arc::new(Rule {
            metric_type: MetricType::Concurrency,
            threshold: 0.5,
            ..Default::default()
        });
        let (r, _, v) = can_pass_check(&rule);
        assert!(r);
        assert!(v.is_none());
    }

    #[test]
    #[ignore]
    fn invalid_concurrency() {
        let rule = Arc::new(Rule {
            metric_type: MetricType::Concurrency,
            threshold: 0.5,
            ..Default::default()
        });
        stat::inbound_node().increase_concurrency();
        let (r, _, v) = can_pass_check(&rule);
        stat::inbound_node().decrease_concurrency();
        assert!(!r);
        assert!(
            (1.0 - *Arc::downcast::<f64>(v.unwrap().as_any_arc()).unwrap()).abs() < f64::EPSILON
        );
    }

    #[test]
    #[ignore]
    fn valid_load() {
        let rule = Arc::new(Rule {
            metric_type: MetricType::Load,
            threshold: 0.5,
            ..Default::default()
        });
        system_metric::set_system_load(0.2);
        let (r, _, v) = can_pass_check(&rule);
        assert!(r);
        assert!(
            (0.2 - *Arc::downcast::<f64>(v.unwrap().as_any_arc()).unwrap()).abs() < f64::EPSILON
        );
        system_metric::set_system_load(0.0);
    }

    #[test]
    #[ignore]
    fn bbr_valid_load() {
        let rule = Arc::new(Rule {
            metric_type: MetricType::Load,
            threshold: 0.5,
            strategy: AdaptiveStrategy::BBR,
            ..Default::default()
        });
        system_metric::set_system_load(1.0);
        stat::inbound_node().increase_concurrency();
        let (r, _, v) = can_pass_check(&rule);
        stat::inbound_node().decrease_concurrency();
        assert!(r);
        assert!(
            (1.0 - *Arc::downcast::<f64>(v.unwrap().as_any_arc()).unwrap()).abs() < f64::EPSILON
        );
        system_metric::set_system_load(0.0);
    }

    #[test]
    #[ignore]
    fn valid_cpu() {
        let rule = Arc::new(Rule {
            metric_type: MetricType::CpuUsage,
            threshold: 0.5,
            ..Default::default()
        });
        system_metric::set_cpu_usage(0.0);
        let (r, _, _) = can_pass_check(&rule);
        assert!(r)
    }

    #[test]
    #[ignore]
    fn bbr_valid_cpu() {
        let rule = Arc::new(Rule {
            metric_type: MetricType::CpuUsage,
            threshold: 0.5,
            strategy: AdaptiveStrategy::BBR,
            ..Default::default()
        });
        system_metric::set_cpu_usage(0.8);
        let (r, _, v) = can_pass_check(&rule);
        assert!(r);
        const DELTA: f64 = 0.0001;
        let snapshot = *Arc::downcast::<f64>(v.unwrap().as_any_arc()).unwrap();
        assert!(snapshot > 0.8 - DELTA && snapshot < 0.8 + DELTA);
        system_metric::set_cpu_usage(0.0);
    }
}
