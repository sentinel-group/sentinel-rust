use crate::{base::SentinelRule, logging, system_metric};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum MetricType {
    /// Load represents system load1 in Linux/Unix.
    Load,
    /// AvgRT represents the average response time of all inbound requests.
    AvgRT,
    /// Concurrency represents the concurrency of all inbound requests.
    Concurrency,
    /// InboundQPS represents the QPS of all inbound requests.
    InboundQPS,
    /// CpuUsage represents the CPU usage percentage of the system.
    CpuUsage,
}

impl Default for MetricType {
    fn default() -> MetricType {
        MetricType::Load
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum AdaptiveStrategy {
    NoAdaptive,
    /// BBR represents the adaptive strategy based on ideas of TCP BBR.
    BBR,
}

impl Default for AdaptiveStrategy {
    fn default() -> AdaptiveStrategy {
        AdaptiveStrategy::NoAdaptive
    }
}

/// `Rule` describes the policy for system resiliency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Rule {
    /// `id` represents the unique ID of the rule (optional).
    pub id: String,
    /// `metric_type` indicates the type of the trigger metric.
    pub metric_type: MetricType,
    /// `threshold` represents the lower bound trigger of the adaptive strategy.
    /// Adaptive strategies will not be activated until target metric has reached the trigger count.
    pub threshold: f64,
    /// `strategy` represents the adaptive strategy.
    pub strategy: AdaptiveStrategy,
}

impl Default for Rule {
    fn default() -> Self {
        Rule {
            id: uuid::Uuid::new_v4().to_string(),
            metric_type: MetricType::default(),
            threshold: 0.0,
            strategy: AdaptiveStrategy::default(),
        }
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.metric_type == other.metric_type
            && self.threshold == other.threshold
            && self.strategy == other.strategy
    }
}

impl SentinelRule for Rule {
    fn resource_name(&self) -> String {
        format!("{:?}", self.metric_type)
    }

    fn is_valid(&self) -> Result<()> {
        if self.threshold < 0.0 {
            return Err(Error::msg("negative threshold"));
        }
        if self.metric_type == MetricType::CpuUsage
            && (self.threshold > 100.0 || self.threshold < 0.0)
        {
            return Err(Error::msg("invalid CPU usage, valid range is [0.0, 100.0]"));
        }
        if self.metric_type == MetricType::Load && (self.threshold > 1.0 || self.threshold < 0.0) {
            return Err(Error::msg(
                "invalid average load, valid range is [0.0, 1.0]",
            ));
        }
        Ok(())
    }
}

impl Hash for Rule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.metric_type.hash(state);
    }
}

impl Eq for Rule {}

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
    #[should_panic(expected = "negative threshold")]
    fn invalid_threshold() {
        let rule = Rule {
            metric_type: MetricType::InboundQPS,
            threshold: -1.0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid CPU usage, valid range is [0.0, 100.0]")]
    fn invalid_cpu_usage() {
        let rule = Rule {
            metric_type: MetricType::CpuUsage,
            threshold: 115.0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }
}
