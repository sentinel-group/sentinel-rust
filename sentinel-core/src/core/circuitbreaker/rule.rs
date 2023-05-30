use super::*;
use crate::{base::SentinelRule, logging, Error};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;
use std::hash::{Hash, Hasher};
cfg_k8s! {
    use schemars::JsonSchema;
    use kube::CustomResource;
}

/// Rule encompasses the fields of circuit breaking rule.
#[cfg_attr(
    feature = "ds_k8s",
    derive(CustomResource, JsonSchema),
    kube(
        group = "rust.datasource.sentinel.io",
        version = "v1alpha1",
        kind = "CircuitBreakerResource",
        namespaced
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// unique id
    pub id: String,
    /// resource name
    pub resource: String,
    pub strategy: BreakerStrategy,
    /// `retry_timeout_ms` represents recovery timeout (in milliseconds) before the circuit breaker opens.
    /// During the open period, no requests are permitted until the timeout has elapsed.
    /// After that, the circuit breaker will transform to half-open state for trying a few "trial" requests.
    pub retry_timeout_ms: u32,
    /// min_request_amount represents the minimum number of requests (in an active statistic time span)
    /// that can trigger circuit breaking.
    pub min_request_amount: u64,
    /// stat_interval_ms represents statistic time interval of the internal circuit breaker (in ms).
    /// Currently the statistic interval is collected by sliding window.
    pub stat_interval_ms: u32,
    /// `stat_sliding_window_bucket_count` represents the bucket count of statistic sliding window.
    /// The statistic will be more precise as the bucket count increases, but the memory cost increases too.
    /// The following must be true — `stat_interval_ms % stat_sliding_window_bucket_count == 0`,
    /// otherwise `stat_sliding_window_bucket_count` will be replaced by 1.
    /// If it is not set, default value 1 will be used.
    pub stat_sliding_window_bucket_count: u32,
    /// `max_allowed_rt_ms` indicates that any invocation whose response time exceeds this value (in ms)
    /// will be recorded as a slow request.
    /// `max_allowed_rt_ms` only takes effect for `SlowRequestRatio` strategy
    pub max_allowed_rt_ms: u64,
    /// `threshold` represents the threshold of circuit breaker.
    /// for `SlowRequestRatio`, it represents the max slow request ratio
    /// for `ErrorRatio`, it represents the max error request ratio
    /// for `ErrorCount`, it represents the max error request count
    pub threshold: f64,
}

impl Default for Rule {
    fn default() -> Self {
        Rule {
            #[cfg(target_arch = "wasm32")]
            id: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            id: uuid::Uuid::new_v4().to_string(),
            resource: String::default(),
            strategy: BreakerStrategy::default(),
            retry_timeout_ms: 0,
            min_request_amount: 0,
            stat_interval_ms: 0,
            stat_sliding_window_bucket_count: 0,
            max_allowed_rt_ms: 0,
            threshold: 0.0,
        }
    }
}

impl Rule {
    pub fn is_stat_reusable(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.strategy == other.strategy
            && self.stat_interval_ms == other.stat_interval_ms
            && self.stat_sliding_window_bucket_count == other.stat_sliding_window_bucket_count
    }

    pub fn get_rule_stat_sliding_window_bucket_count(&self) -> u32 {
        let mut bucket_count = self.stat_sliding_window_bucket_count;
        if bucket_count == 0 || self.stat_interval_ms % bucket_count != 0 {
            bucket_count = 1
        }
        bucket_count
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
        if self.stat_interval_ms == 0 {
            return Err(Error::msg("invalid stat_interval_ms"));
        }
        if self.retry_timeout_ms == 0 {
            return Err(Error::msg("invalid retry_timeout_ms"));
        }
        if self.threshold < 0.0 {
            return Err(Error::msg("invalid threshold"));
        }
        if self.strategy != BreakerStrategy::ErrorCount && self.threshold > 1.0 {
            return Err(Error::msg(format!(
                "invalid {:?} ratio threshold (valid range: [0.0, 1.0])",
                self.strategy
            )));
        }
        if self.stat_sliding_window_bucket_count != 0
            && self.stat_interval_ms % self.stat_sliding_window_bucket_count != 0
        {
            logging::warn!("[CircuitBreaker IsValidRule] The following must be true: stat_interval_ms % stat_sliding_window_bucket_count == 0. stat_sliding_window_bucket_count will be replaced by 1, rule {:?}", self);
        }
        Ok(())
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.strategy == other.strategy
            && self.retry_timeout_ms == other.retry_timeout_ms
            && self.min_request_amount == other.min_request_amount
            && self.stat_interval_ms == other.stat_interval_ms
            && self.stat_sliding_window_bucket_count == other.stat_sliding_window_bucket_count
            && match self.strategy {
                BreakerStrategy::SlowRequestRatio => {
                    self.max_allowed_rt_ms == other.max_allowed_rt_ms
                        && self.threshold == other.threshold
                }
                _ => self.threshold == other.threshold,
            }
    }
}

impl Hash for Rule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.resource.hash(state);
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
    fn test_reusable() {
        let rules = vec![
            // different resource
            (
                Rule {
                    resource: "abc".into(),
                    ..Default::default()
                },
                Rule {
                    resource: "def".into(),
                    ..Default::default()
                },
                false,
                false,
            ),
            // different strategy
            (
                Rule {
                    strategy: BreakerStrategy::ErrorCount,
                    ..Default::default()
                },
                Rule {
                    strategy: BreakerStrategy::ErrorRatio,
                    ..Default::default()
                },
                false,
                false,
            ),
            // different stat_interval_ms
            (
                Rule {
                    stat_interval_ms: 10000,
                    ..Default::default()
                },
                Rule {
                    stat_interval_ms: 5000,
                    ..Default::default()
                },
                false,
                false,
            ),
            // different stat_sliding_window_bucket_count
            (
                Rule {
                    stat_sliding_window_bucket_count: 2,
                    ..Default::default()
                },
                Rule {
                    stat_sliding_window_bucket_count: 5,
                    ..Default::default()
                },
                false,
                false,
            ),
            // different retry_timeout_ms
            (
                Rule {
                    retry_timeout_ms: 3000,
                    ..Default::default()
                },
                Rule {
                    retry_timeout_ms: 5000,
                    ..Default::default()
                },
                true,
                false,
            ),
            // different min_request_amount
            (
                Rule {
                    min_request_amount: 10,
                    ..Default::default()
                },
                Rule {
                    min_request_amount: 20,
                    ..Default::default()
                },
                true,
                false,
            ),
            // different threshold
            (
                Rule {
                    threshold: 1.0,
                    ..Default::default()
                },
                Rule {
                    threshold: 2.0,
                    ..Default::default()
                },
                true,
                false,
            ),
            // different max_allowed_rt_ms on BreakerStrategy::ErrorCount
            (
                Rule {
                    strategy: BreakerStrategy::ErrorCount,
                    max_allowed_rt_ms: 1000,
                    ..Default::default()
                },
                Rule {
                    strategy: BreakerStrategy::ErrorCount,
                    max_allowed_rt_ms: 2000,
                    ..Default::default()
                },
                true,
                true,
            ),
            // different max_allowed_rt_ms on BreakerStrategy::SlowRequestRatio
            (
                Rule {
                    strategy: BreakerStrategy::SlowRequestRatio,
                    max_allowed_rt_ms: 1000,
                    ..Default::default()
                },
                Rule {
                    strategy: BreakerStrategy::SlowRequestRatio,
                    max_allowed_rt_ms: 2000,
                    ..Default::default()
                },
                true,
                false,
            ),
        ];
        for (r1, r2, reuse_expected, eq_expected) in rules {
            assert_eq!(r1.is_stat_reusable(&r2), reuse_expected);
            assert_eq!(r1 == r2, eq_expected);
        }
    }

    #[test]
    fn test_bucket_count() {
        let rules = vec![
            // count == 1
            (
                Rule {
                    stat_interval_ms: 1000,
                    ..Default::default()
                },
                1,
            ),
            (
                Rule {
                    stat_sliding_window_bucket_count: 1,
                    stat_interval_ms: 1000,
                    ..Default::default()
                },
                1,
            ),
            (
                Rule {
                    stat_sliding_window_bucket_count: 10,
                    stat_interval_ms: 1000,
                    ..Default::default()
                },
                10,
            ),
            (
                Rule {
                    stat_sliding_window_bucket_count: 30,
                    stat_interval_ms: 1000,
                    ..Default::default()
                },
                1,
            ),
            (
                Rule {
                    stat_sliding_window_bucket_count: 100,
                    stat_interval_ms: 100,
                    ..Default::default()
                },
                100,
            ),
            (
                Rule {
                    stat_sliding_window_bucket_count: 200,
                    stat_interval_ms: 100,
                    ..Default::default()
                },
                1,
            ),
        ];
        for (rule, expected) in rules {
            assert_eq!(rule.get_rule_stat_sliding_window_bucket_count(), expected);
        }
    }

    #[test]
    fn test_valid() {
        let rules = vec![
            Rule {
                resource: "abc".into(),
                strategy: BreakerStrategy::SlowRequestRatio,
                retry_timeout_ms: 1000,
                min_request_amount: 5,
                stat_interval_ms: 1000,
                max_allowed_rt_ms: 20,
                threshold: 0.1,
                ..Default::default()
            },
            Rule {
                resource: "abc".into(),
                strategy: BreakerStrategy::ErrorRatio,
                retry_timeout_ms: 1000,
                min_request_amount: 5,
                stat_interval_ms: 1000,
                threshold: 0.3,
                ..Default::default()
            },
            Rule {
                resource: "abc".into(),
                strategy: BreakerStrategy::ErrorCount,
                retry_timeout_ms: 1000,
                min_request_amount: 5,
                stat_interval_ms: 1000,
                threshold: 10.0,
                ..Default::default()
            },
        ];
        for rule in rules {
            assert!(rule.is_valid().is_ok());
        }
    }

    #[test]
    #[should_panic(expected = "empty resource name")]
    fn illegal1() {
        let rule = Rule::default();
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid stat_interval_ms")]
    fn illegal2() {
        let rule = Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            stat_interval_ms: 0,
            threshold: 3.0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid retry_timeout_ms")]
    fn illegal3() {
        let rule = Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 0,
            stat_interval_ms: 1000,
            threshold: 3.0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid threshold")]
    fn illegal4() {
        let rule = Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            stat_interval_ms: 1000,
            threshold: -4.0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid SlowRequestRatio ratio threshold (valid range: [0.0, 1.0])")]
    fn illegal5() {
        let rule = Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            stat_interval_ms: 1000,
            threshold: 2.0,
            ..Default::default()
        };
        rule.is_valid().unwrap();
    }
}
