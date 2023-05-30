use crate::{
    base::{ParamKey, SentinelRule},
    Error,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
cfg_k8s! {
    use schemars::JsonSchema;
    use kube::CustomResource;
}

/// ControlStrategy indicates the traffic shaping strategy.
#[cfg_attr(feature = "ds_k8s", derive(JsonSchema))]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum ControlStrategy {
    Reject,
    Throttling,
    #[serde(skip)]
    Custom(u8),
}

impl Default for ControlStrategy {
    fn default() -> Self {
        ControlStrategy::Reject
    }
}

// MetricType represents the target metric type.
#[cfg_attr(feature = "ds_k8s", derive(JsonSchema))]
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
#[cfg_attr(
    feature = "ds_k8s",
    derive(CustomResource, JsonSchema),
    kube(
        group = "rust.datasource.sentinel.io",
        version = "v1alpha1",
        kind = "HotspotResource",
        namespaced
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// `id` is the unique id
    pub id: String,
    /// `resource` is the resource name
    pub resource: String,
    /// `metric_type` indicates the metric type for checking logic.
    /// For Concurrency metric, hotspot module will check the each hot parameter's concurrency,
    /// if concurrency exceeds the threshold, reject the traffic directly.
    /// For QPS metric, hotspot module will check the each hot parameter's QPS,
    /// the `control_strategy` decides the behavior of traffic shaping controller
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
    /// max_queueing_time_ms only takes effect when `control_strategy` is `Throttling` and `metric_type` is `QPS`
    pub max_queueing_time_ms: u64,
    /// `burst_count` is the silent count
    /// `burst_count` only takes effect when `control_strategy` is `Reject` and `metric_type` is `QPS`
    pub burst_count: u64,
    /// `duration_in_sec` is the time interval in statistic
    /// `duration_in_sec` only takes effect when `metric_type` is QPS
    pub duration_in_sec: u64,
    /// `params_max_capacity` is the max capacity of cache statistic
    pub params_max_capacity: usize,
    /// `specific_items` indicates the special threshold for specific value
    pub specific_items: HashMap<ParamKey, u64>,
}

impl Default for Rule {
    fn default() -> Self {
        Rule {
            #[cfg(target_arch = "wasm32")]
            id: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            id: uuid::Uuid::new_v4().to_string(),
            resource: String::default(),
            metric_type: MetricType::default(),
            control_strategy: ControlStrategy::default(),
            param_index: 0,
            param_key: String::default(),
            threshold: 0,
            max_queueing_time_ms: 0,
            burst_count: 0,
            duration_in_sec: 0,
            params_max_capacity: 0,
            specific_items: HashMap::default(),
        }
    }
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

impl Eq for Rule {}

impl Hash for Rule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.resource.hash(state);
    }
}

impl SentinelRule for Rule {
    fn resource_name(&self) -> String {
        self.resource.clone()
    }

    fn is_valid(&self) -> crate::Result<()> {
        if self.resource.is_empty() {
            return Err(Error::msg("empty resource name"));
        }
        if self.metric_type == MetricType::QPS && self.duration_in_sec == 0 {
            return Err(Error::msg("invalid duration"));
        }
        if self.param_index > 0 && !self.param_key.is_empty() {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic(expected = "empty resource name")]
    fn invalid_name() {
        let rule = Rule::default();
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid duration")]
    fn invalid_duration() {
        let rule = Rule {
            resource: "name".into(),
            metric_type: MetricType::QPS,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "param index and param key are mutually exclusive")]
    fn invalid_param() {
        let rule = Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            param_index: 10,
            param_key: "test2".into(),
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    fn test_eq() {
        let mut specific_items: HashMap<ParamKey, u64> = HashMap::new();
        specific_items.insert("sss".into(), 1);
        specific_items.insert("1123".into(), 3);
        let rule1 = Rule {
            id: "abc".into(),
            resource: "abc".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            param_index: 0,
            param_key: "key".into(),
            threshold: 110,
            max_queueing_time_ms: 5,
            burst_count: 10,
            duration_in_sec: 1,
            params_max_capacity: 10000,
            specific_items: specific_items.clone(),
        };
        let rule2 = Rule {
            id: "abc".into(),
            resource: "abc".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            param_index: 0,
            param_key: "key".into(),
            threshold: 110,
            max_queueing_time_ms: 5,
            burst_count: 10,
            duration_in_sec: 1,
            params_max_capacity: 10000,
            specific_items,
        };
        assert_eq!(rule1, rule2);
    }
}
