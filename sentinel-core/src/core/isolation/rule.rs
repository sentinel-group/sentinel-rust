use crate::{base::SentinelRule, Error};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;
use std::hash::{Hash, Hasher};
cfg_k8s! {
    use schemars::JsonSchema;
    use kube::CustomResource;
}

#[cfg_attr(feature = "ds_k8s", derive(JsonSchema))]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum MetricType {
    /// Concurrency represents the concurrency of in-flight requests
    Concurrency,
}

impl Default for MetricType {
    fn default() -> MetricType {
        MetricType::Concurrency
    }
}

/// `Rule` describes the policy for system resiliency.
#[cfg_attr(
    feature = "ds_k8s",
    derive(CustomResource, JsonSchema),
    kube(
        group = "rust.datasource.sentinel.io",
        version = "v1alpha1",
        kind = "IsolationResource",
        namespaced
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// `id` represents the unique ID of the rule (optional).
    pub id: String,
    /// `resource` represents the target resource definition
    pub resource: String,
    /// `metric_type` indicates the type of the trigger metric.
    pub metric_type: MetricType,
    pub threshold: u32,
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
            threshold: 0,
        }
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.metric_type == other.metric_type
            && self.threshold == other.threshold
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
        format!("{:?}", self.metric_type)
    }

    fn is_valid(&self) -> crate::Result<()> {
        if self.resource.is_empty() {
            return Err(Error::msg("empty resource of isolation rule"));
        }

        if self.threshold == 0 {
            return Err(Error::msg("zero threshold"));
        }
        Ok(())
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
    #[should_panic(expected = "zero threshold")]
    fn invalid_threshold() {
        let rule = Rule {
            resource: "invalid_threshold".into(),
            threshold: 0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "empty resource of isolation rule")]
    fn invalid_cpu_usage() {
        let rule = Rule {
            threshold: 1,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }
}
