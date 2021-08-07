use crate::{core::base::SentinelRule, core::system_metric, logging};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;

/// RelationStrategy indicates the flow control strategy based on the relation of invocations.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum RelationStrategy {
    /// CurrentResource means flow control by current resource directly.
    CurrentResource,
    /// AssociatedResource means flow control by the associated resource rather than current resource.
    AssociatedResource,
}

impl Default for RelationStrategy {
    fn default() -> RelationStrategy {
        RelationStrategy::CurrentResource
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum CalculateStrategy {
    Direct,
    WarmUp,
    MemoryAdaptive,
    Custom(u8),
}

impl Default for CalculateStrategy {
    fn default() -> CalculateStrategy {
        CalculateStrategy::Direct
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum ControlStrategy {
    Reject,
    /// Throttling indicates that pending requests will be throttled,
    /// wait in queue (until free capacity is available)
    Throttling,
    Custom(u8),
}

impl Default for ControlStrategy {
    fn default() -> ControlStrategy {
        ControlStrategy::Reject
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
/// Rule describes the strategy of flow control, the flow control strategy is based on QPS statistic metric
pub struct Rule {
    /// `id` represents the unique ID of the rule (optional).
    pub id: Option<String>,
    /// `resource` represents the resource name.
    pub resource: String,
    pub ref_resource: String,
    pub calculate_strategy: CalculateStrategy,
    pub control_strategy: ControlStrategy,
    pub relation_strategy: RelationStrategy,
    /// `threshold` means the threshold during stat_interval_ms
    /// If `stat_interval_ms` is 1000(1 second), `threshold` means QPS
    pub threshold: f64,
    pub warm_up_period_sec: u32,
    pub warm_up_cold_factor: u32,
    /// `max_queueing_time_ms` only takes effect when `control_strategy` is Throttling.
    /// When `max_queueing_time_ms` is 0, it means Throttling only controls interval of requests,
    /// and requests exceeding the threshold will be rejected directly.
    pub max_queueing_time_ms: u32,
    /// stat_interval_ms indicates the statistic interval and it's the optional setting for flow Rule.
    /// If user doesn't set stat_interval_ms, that means using default metric statistic of resource.
    /// If the stat_interval_ms user specifies can not reuse the global statistic of resource,
    /// sentinel will generate independent statistic structure for this self.
    pub stat_interval_ms: u32,

    /// adaptive flow control algorithm related parameters
    /// limitation: low_mem_usage_threshold > high_mem_usage_threshold && mem_high_water_mark > mem_low_water_mark
    /// - if the current memory usage is less than or equals to mem_low_water_mark, threshold == low_mem_usage_threshold
    /// - if the current memory usage is more than or equals to mem_high_water_mark, threshold == high_mem_usage_threshold
    /// - if the current memory usage is in (mem_low_water_mark, mem_high_water_mark), threshold is in (high_mem_usage_threshold, low_mem_usage_threshold)
    pub low_mem_usage_threshold: u64,
    pub high_mem_usage_threshold: u64,
    pub mem_low_water_mark: u64,
    pub mem_high_water_mark: u64,
}

impl Rule {
    pub fn is_stat_reusable(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.relation_strategy == other.relation_strategy
            && self.ref_resource == other.ref_resource
            && self.stat_interval_ms == other.stat_interval_ms
            && self.need_statistic()
            && other.need_statistic()
    }

    pub fn need_statistic(&self) -> bool {
        return self.calculate_strategy == CalculateStrategy::WarmUp
            || self.control_strategy == ControlStrategy::Reject;
    }

    pub fn is_valid(&self) -> Result<()> {
        if self.resource.len() == 0 {
            return Err(Error::msg("empty resource name"));
        }
        if self.threshold < 0.0 {
            return Err(Error::msg("negative threshold"));
        }
        if self.relation_strategy == RelationStrategy::AssociatedResource
            && self.ref_resource.len() == 0
        {
            return Err(Error::msg("ref_resource must be non empty when relation_strategy is RelationStrategy::AssociatedResource"));
        }
        if self.calculate_strategy == CalculateStrategy::WarmUp {
            if self.warm_up_period_sec == 0 {
                return Err(Error::msg("warm_up_period_sec must be great than 0"));
            }
            if self.warm_up_cold_factor == 1 {
                return Err(Error::msg("warm_up_cold_factor must be great than 1"));
            }
        }
        if self.stat_interval_ms > 10 * 60 * 1000 {
            logging::info!(
                "stat_interval_ms is great than 10 minutes, less than 10 minutes is recommended."
            )
        }
        if self.calculate_strategy == CalculateStrategy::MemoryAdaptive {
            if self.mem_low_water_mark == 0
                || self.mem_high_water_mark == 0
                || self.high_mem_usage_threshold == 0
                || self.low_mem_usage_threshold == 0
            {
                return Err(Error::msg(
                    "memory water mark or usage threshold setting to 0",
                ));
            }
            if self.high_mem_usage_threshold >= self.low_mem_usage_threshold {
                return Err(Error::msg(
                    "self.high_mem_usage_threshold >= self.low_mem_usage_threshold",
                ));
            }
            if self.mem_high_water_mark > system_metric::get_total_memory_size() {
                return Err(Error::msg("self.mem_high_water_mark should not be greater than current system's total memory size"));
            }
            if self.mem_low_water_mark >= self.mem_high_water_mark {
                // can not be equal to defeat from zero overflow
                return Err(Error::msg(
                    "self.mem_low_water_mark >= self.mem_high_water_mark",
                ));
            }
        }
        Ok(())
    }
}

impl SentinelRule for Rule {
    fn resource_name(&self) -> String {
        self.resource.clone()
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.ref_resource == other.ref_resource
            && self.calculate_strategy == other.calculate_strategy
            && self.control_strategy == other.control_strategy
            && self.relation_strategy == other.relation_strategy
            && self.threshold == other.threshold
            && self.warm_up_period_sec == other.warm_up_period_sec
            && self.warm_up_cold_factor == other.warm_up_cold_factor
            && self.max_queueing_time_ms == other.max_queueing_time_ms
            && self.stat_interval_ms == other.stat_interval_ms
            && self.low_mem_usage_threshold == other.low_mem_usage_threshold
            && self.high_mem_usage_threshold == other.high_mem_usage_threshold
            && self.mem_low_water_mark == other.mem_low_water_mark
            && self.mem_high_water_mark == other.mem_high_water_mark
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmtted = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", fmtted)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn need_statistic() {
        // need
        let r1 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        // no need
        let r2 = Rule {
            resource: "abc1".into(),
            threshold: 200.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Throttling,
            max_queueing_time_ms: 10,
            stat_interval_ms: 2000,
            ..Default::default()
        };
        // need
        let r3 = Rule {
            resource: "abc1".into(),
            threshold: 300.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::WarmUp,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 5000,
            ..Default::default()
        };
        // need
        let r4 = Rule {
            resource: "abc1".into(),
            threshold: 400.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::WarmUp,
            control_strategy: ControlStrategy::Throttling,
            max_queueing_time_ms: 10,
            stat_interval_ms: 50000,
            ..Default::default()
        };

        assert!(r1.need_statistic());
        assert!(!r2.need_statistic());
        assert!(r3.need_statistic());
        assert!(r4.need_statistic());
    }

    #[test]
    fn is_stat_reusable() {
        // Not same resource
        let r11 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        let r12 = Rule {
            resource: "abc2".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        assert!(!r11.is_stat_reusable(&r12));

        // Not same relation strategy
        let r21 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        let r22 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::AssociatedResource,
            ref_resource: "abc3".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        assert!(!r21.is_stat_reusable(&r22));

        // Not same ref resource
        let r31 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::AssociatedResource,
            ref_resource: "abc3".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        let r32 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::AssociatedResource,
            ref_resource: "abc4".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        assert!(!r31.is_stat_reusable(&r32));

        // Not same stat interval
        let r41 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        let r42 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 2000,
            ..Default::default()
        };
        assert!(!r41.is_stat_reusable(&r42));

        // Not both need stat
        let r51 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        let r52 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Throttling,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        assert!(!r51.is_stat_reusable(&r52));

        // Not same threshold
        let r61 = Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        let r62 = Rule {
            resource: "abc1".into(),
            threshold: 200.0,
            relation_strategy: RelationStrategy::CurrentResource,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        };
        assert!(r61.is_stat_reusable(&r62));
    }
}
