use super::*;
use crate::{base::SentinelRule, logging, system_metric, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;

/// Rule encompasses the fields of circuit breaking rule.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// unique id
    pub id: Option<String>,
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

impl Rule {
    pub fn is_stat_reusable(&self, other: &Self) -> bool {
        self.resource == other.resource
            && self.strategy == other.strategy
            && self.stat_interval_ms == other.stat_interval_ms
            && self.stat_sliding_window_bucket_count == other.stat_sliding_window_bucket_count
    }

    pub fn get_rule_stat_sliding_window_bucket_count(&self) -> u32 {
        let interval = self.stat_interval_ms;
        let mut bucket_count = self.stat_sliding_window_bucket_count;
        if bucket_count == 0 || interval % bucket_count != 0 {
            bucket_count = 1
        }
        return bucket_count;
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
        if self.resource == other.resource
            && self.strategy == other.strategy
            && self.retry_timeout_ms == other.retry_timeout_ms
            && self.min_request_amount == other.min_request_amount
            && self.stat_interval_ms == other.stat_interval_ms
            && self.stat_sliding_window_bucket_count == other.stat_sliding_window_bucket_count
        {
            match self.strategy {
                BreakerStrategy::SlowRequestRatio => {
                    self.max_allowed_rt_ms == other.max_allowed_rt_ms
                        && self.threshold == other.threshold
                }
                _ => self.threshold == other.threshold,
            }
        } else {
            false
        }
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
}
