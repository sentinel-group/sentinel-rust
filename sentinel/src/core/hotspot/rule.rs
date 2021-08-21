use crate::{
    base::{ParamKey, SentinelRule},
    logging, system_metric, Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::hash::Hash;

/// ControlStrategy indicates the traffic shaping strategy.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum ControlStrategy {
    Reject,
    Throttling,
    Custom(u8),
}

impl Default for ControlStrategy {
    fn default() -> Self {
        ControlStrategy::Reject
    }
}

// MetricType represents the target metric type.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetricType {
    /// Concurrency represents concurrency count.
    Concurrency,
    /// QPS represents request count per second.
    QPS,
}

impl Default for MetricType {
    fn default() -> Self {
        MetricType::Concurrency
    }
}

/// Rule represents the hotspot(frequent) parameter flow control rule
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// `id` is the unique id
    pub id: Option<String>,
    /// `resource` is the resource name
    pub resource: String,
    /// `metric_type` indicates the metric type for checking logic.
    /// For Concurrency metric, hotspot module will check the each hot parameter's concurrency,
    ///		if concurrency exceeds the threshold, reject the traffic directly.
    /// For QPS metric, hotspot module will check the each hot parameter's QPS,
    ///		the `control_strategy` decides the behavior of traffic shaping controller
    pub metric_type: MetricType,
    /// `control_strategy` indicates the traffic shaping behaviour.
    /// `control_strategy` only takes effect when `metric_type` is QPS
    pub control_strategy: ControlStrategy,
    /// ``param_index`` is the index in context arguments slice.
    /// `param_index` means the <`param_index`>-th parameter
    pub param_index: isize,
    /// `param_key` is the key in EntryContext.Input.Attachments map.
    /// `param_key` can be used as a supplement to `param_index` to facilitate rules to quickly obtain parameter from a large number of parameters
    /// `param_key` is mutually exclusive with `param_index`, `param_key` has the higher priority than `param_index`
    pub param_key: String,
    /// threshold is the threshold to trigger rejection
    pub threshold: u64,
    /// max_queueing_time_ms only takes effect when control_strategy is Throttling and `metric_type` is QPS
    pub max_queueing_time_ms: u64,
    /// `burst_count` is the silent count
    /// `burst_count` only takes effect when control_strategy is Reject and `metric_type` is QPS
    pub burst_count: u64,
    /// `duration_in_sec` is the time interval in statistic
    /// `duration_in_sec` only takes effect when `metric_type` is QPS
    pub duration_in_sec: u64,
    /// `params_max_capacity` is the max capacity of cache statistic
    pub params_max_capacity: usize,
    /// `specific_items` indicates the special threshold for specific value
    pub specific_items: HashMap<ParamKey, u64>,
}

impl Rule {
    pub fn is_stat_reusable(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.control_strategy == other.control_strategy
            && self.params_max_capacity == other.params_max_capacity
            && self.duration_in_sec == other.duration_in_sec
            && self.metric_type == other.metric_type
    }
}

impl SentinelRule for Rule {
    fn resource_name(&self) -> String {
        self.resource.clone()
    }

    fn is_valid(&self) -> Result<()> {
        if self.resource.len() == 0 {
            return Err(Error::msg("empty resource name"));
        }
        if self.metric_type == MetricType::QPS && self.duration_in_sec == 0 {
            return Err(Error::msg("invalid duration"));
        }
        if self.param_index > 0 && self.param_key.len() != 0 {
            return Err(Error::msg(
                "param index and param key are mutually exclusive",
            ));
        }
        Ok(())
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.metric_type == other.metric_type
            && self.control_strategy == other.control_strategy
            && self.params_max_capacity == other.params_max_capacity
            && self.param_index == other.param_index
            && self.param_key == other.param_key
            && self.threshold == other.threshold
            && self.duration_in_sec == other.duration_in_sec
            && self.specific_items == other.specific_items
            && ((self.control_strategy == ControlStrategy::Reject
                && self.burst_count == other.burst_count)
                || (self.control_strategy == ControlStrategy::Throttling
                    && self.max_queueing_time_ms == other.max_queueing_time_ms))
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmtted = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", fmtted)
    }
}
