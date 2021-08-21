//! Stat
//!
use super::MetricItemRetriever;
use crate::{utils::AsAny, Error, Result};
use enum_map::Enum;
use lazy_static::lazy_static;
use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex};

pub type TimePredicate = dyn Fn(u64) -> bool;

/// There are five events to record
/// pass + block == Total
#[derive(Debug, Clone, Copy, Enum)]
pub enum MetricEvent {
    /// sentinel rules check pass
    Pass,
    /// sentinel rules check block
    Block,
    Complete,
    /// Biz error, used for circuit breaker
    Error,
    /// request execute Round Trip Time, unit is millisecond
    Rt,
}

// todo: consider use the static reference, do not create Arc pointer?
lazy_static! {
    static ref NOP_READ_STAT: Arc<NopReadStat> = Arc::new(NopReadStat {});
    static ref NOP_WRITE_STAT: Arc<NopWriteStat> = Arc::new(NopWriteStat {});
}

#[inline]
pub fn nop_read_stat() -> Arc<NopReadStat> {
    NOP_READ_STAT.clone()
}

#[inline]
pub fn nop_write_stat() -> Arc<NopWriteStat> {
    NOP_WRITE_STAT.clone()
}

pub const ILLEGAL_GLOBAL_STATISTIC_PARAMS_ERROR: &str =
    "Invalid parameters, sampleCount or interval, for resource's global statistic";
pub const ILLEGAL_STATISTIC_PARAMS_ERROR: &str =
    "Invalid parameters, sampleCount or interval, for metric statistic";
pub const GLOBAL_STATISTIC_NON_REUSABLE_ERROR : &str  = "The parameters, sampleCount and interval, mismatch for reusing between resource's global statistic and readonly metric statistic.";

pub trait ReadStat: Send + Sync + fmt::Debug {
    fn qps(&self, _event: MetricEvent) -> f64 {
        0f64
    }
    fn qps_previous(&self, _event: MetricEvent) -> f64 {
        0f64
    }
    fn sum(&self, _event: MetricEvent) -> u64 {
        0u64
    }
    fn min_rt(&self) -> f64 {
        0f64
    }
    fn avg_rt(&self) -> f64 {
        0f64
    }
}

pub trait WriteStat: Send + Sync + fmt::Debug {
    fn add_count(&self, _event: MetricEvent, _count: u64) {}
    fn update_concurrency(&self, _concurrency: u32) {}
}

pub trait ConcurrencyStat: Send + Sync + fmt::Debug {
    fn current_concurrency(&self) -> u32;
    fn increase_concurrency(&self);
    fn decrease_concurrency(&self);
}

#[derive(Debug)]
pub struct NopReadStat {}
impl ReadStat for NopReadStat {}

#[derive(Debug)]
pub struct NopWriteStat {}
impl WriteStat for NopWriteStat {}

/// StatNode holds real-time statistics for resources.
pub trait StatNode:
    ReadStat + WriteStat + ConcurrencyStat + MetricItemRetriever + Any + AsAny
{
    /// generate_read_stat generates the readonly metric statistic based on resource level global statistic
    /// If parameters, sampleCount and intervalInMs, are not suitable for resource level global statistic, return (nil, error)
    fn generate_read_stat(&self, sample_count: u32, interval_ms: u32) -> Result<Arc<dyn ReadStat>>;
}

pub fn check_validity_for_statistic(
    sample_count: u32,
    interval_ms: u32,
    error_msg: &'static str,
) -> Result<()> {
    if interval_ms == 0 || sample_count == 0 || interval_ms % sample_count != 0 {
        return Err(Error::msg(error_msg));
    }
    Ok(())
}

/// check_validity_for_reuse_statistic check the compliance whether readonly metric statistic can be built based on resource's global statistic
/// The parameters, sample_count and interval_ms, are the parameters of the metric statistic you want to build
/// The parameters, parent_sample_count and parent_interval_ms, are the parameters of the resource's global statistic
/// If compliance passes, return Ok(()), if not returns specific error
pub fn check_validity_for_reuse_statistic(
    sample_count: u32,
    interval_ms: u32,
    parent_sample_count: u32,
    parent_interval_ms: u32,
) -> Result<()> {
    check_validity_for_statistic(sample_count, interval_ms, ILLEGAL_STATISTIC_PARAMS_ERROR)?;
    let bucket_length_in_ms = interval_ms / sample_count;

    check_validity_for_statistic(
        parent_sample_count,
        parent_interval_ms,
        ILLEGAL_GLOBAL_STATISTIC_PARAMS_ERROR,
    )?;
    let parent_bucket_length_in_ms = parent_interval_ms / parent_sample_count;

    //SlidingWindowMetric's intervalInMs is not divisible by BucketLeapArray's intervalInMs
    if parent_interval_ms % interval_ms != 0 {
        return Err(Error::msg(GLOBAL_STATISTIC_NON_REUSABLE_ERROR));
    }
    // BucketLeapArray's BucketLengthInMs is not divisible by SlidingWindowMetric's BucketLengthInMs
    if bucket_length_in_ms % parent_bucket_length_in_ms != 0 {
        return Err(Error::msg(GLOBAL_STATISTIC_NON_REUSABLE_ERROR));
    }
    Ok(())
}

// expose the moudle in crate for possible testing usage
#[cfg(test)]
pub(crate) use test::*;

#[cfg(test)]
mod test {
    use super::super::MetricItem;
    use super::*;
    use mockall::predicate::*;
    use mockall::*;

    mock! {
        #[derive(Debug)]
        pub(crate) StatNode {}
        impl ReadStat for StatNode {
            fn qps(&self, _event: MetricEvent) -> f64;
            fn qps_previous(&self, _event: MetricEvent) -> f64;
            fn sum(&self, _event: MetricEvent) -> u64;
            fn min_rt(&self) -> f64;
            fn avg_rt(&self) -> f64;
        }
        impl WriteStat for StatNode {
            fn add_count(&self, _event: MetricEvent, _count: u64);
            fn update_concurrency(&self, concurrency: u32);
        }
        impl ConcurrencyStat for StatNode {
            fn current_concurrency(&self) -> u32;
            fn increase_concurrency(&self) ;
            fn decrease_concurrency(&self) ;
        }
        impl MetricItemRetriever for StatNode {
            fn metrics_on_condition(&self, predicate: &TimePredicate) -> Vec<MetricItem>;
        }
        impl StatNode for StatNode {
            fn generate_read_stat(
                &self,
                sample_count: u32,
                interval_ms: u32,
            ) -> Result<Arc<dyn ReadStat>> ;
        }
    }

    #[test]
    fn valid() {
        check_validity_for_reuse_statistic(1, 1000, 100, 10000).unwrap();
        check_validity_for_reuse_statistic(2, 1000, 20, 10000).unwrap();
    }

    #[test]
    fn invalid() {
        assert_eq!(
            check_validity_for_reuse_statistic(3, 1000, 20, 10000)
                .unwrap_err()
                .to_string(),
            ILLEGAL_STATISTIC_PARAMS_ERROR
        );
        assert_eq!(
            check_validity_for_reuse_statistic(0, 1000, 20, 10000)
                .unwrap_err()
                .to_string(),
            ILLEGAL_STATISTIC_PARAMS_ERROR
        );
        assert_eq!(
            check_validity_for_reuse_statistic(2, 1000, 21, 10000)
                .unwrap_err()
                .to_string(),
            ILLEGAL_GLOBAL_STATISTIC_PARAMS_ERROR
        );
        assert_eq!(
            check_validity_for_reuse_statistic(2, 1000, 0, 10000)
                .unwrap_err()
                .to_string(),
            ILLEGAL_GLOBAL_STATISTIC_PARAMS_ERROR
        );
        assert_eq!(
            check_validity_for_reuse_statistic(2, 8000, 20, 10000)
                .unwrap_err()
                .to_string(),
            GLOBAL_STATISTIC_NON_REUSABLE_ERROR
        );
        assert_eq!(
            check_validity_for_reuse_statistic(2, 1000, 10, 10000)
                .unwrap_err()
                .to_string(),
            GLOBAL_STATISTIC_NON_REUSABLE_ERROR
        );
    }
}
