//! Stat
//!
use super::MetricItemRetriever;
use crate::{Error, Result};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Mutex;

pub type TimePredicate = fn(u64) -> bool;

/// There are five events to record
/// pass + block == Total
#[derive(Debug, Clone, Copy)]
pub enum MetricEvent {
    /// sentinel rules check pass
    MetricEventPass,
    /// sentinel rules check block
    MetricEventBlock,
    MetricEventComplete,
    /// Biz error, used for circuit breaker
    MetricEventError,
    /// request execute rt, unit is millisecond
    MetricEventRt,
    /// hack for the number of event
    MetricEventTotal,
}

lazy_static! {
    static ref NOP_READ_STAT: Mutex<NopReadStat> = Mutex::new(NopReadStat {});
    static ref NOP_WRITE_STAT: Mutex<NopWriteStat> = Mutex::new(NopWriteStat {});
}

const ILLEGAL_GLOBAL_STATISTIC_PARAMS_ERROR: &str =
    "Invalid parameters, sampleCount or interval, for resource's global statistic";
const ILLEGAL_STATISTIC_PARAMS_ERROR: &str =
    "Invalid parameters, sampleCount or interval, for metric statistic";
const GLOBAL_STATISTIC_NON_REUSABLE_ERROR : &str  = "The parameters, sampleCount and interval, mismatch for reusing between resource's global statistic and readonly metric statistic.";

pub trait ReadStat {
    fn get_qps(&self) -> f64 {
        0f64
    }
    fn get_previous_qps(&self) -> f64 {
        0f64
    }
    fn get_sum(&self) -> i64 {
        0i64
    }
    fn min_rt(&self) -> f64 {
        0f64
    }
    fn avg_rt(&self) -> f64 {
        0f64
    }
}

pub trait WriteStat {
    fn add_count(&self, _metric_event: MetricEvent, _count: i64) {}
}

pub trait ConcurrencyStat {
    fn current_concurrency(&self) -> i32;
    fn increase_concurrency(&self);
    fn decrease_concurrency(&self);
}

pub struct NopReadStat {}
impl ReadStat for NopReadStat {}

pub struct NopWriteStat {}
impl WriteStat for NopWriteStat {}

/// StatNode holds real-time statistics for resources.
pub trait StatNode:
    ReadStat + WriteStat + ConcurrencyStat + MetricItemRetriever + fmt::Debug
{
    /// generate_read_stat generates the readonly metric statistic based on resource level global statistic
    /// If parameters, sampleCount and intervalInMs, are not suitable for resource level global statistic, return (nil, error)
    fn generate_read_stat(
        &self,
        sample_count: u32,
        interval_in_ms: u32,
    ) -> Result<Rc<RefCell<dyn ReadStat>>>;
}

pub fn check_validity_for_statistic(
    sample_count: u32,
    interval_in_ms: u32,
    error_msg: &'static str,
) -> Result<()> {
    if interval_in_ms == 0 || sample_count == 0 || interval_in_ms % sample_count != 0 {
        return Err(Error::msg(error_msg));
    }
    Ok(())
}

/// check_validity_for_reuse_statistic check the compliance whether readonly metric statistic can be built based on resource's global statistic
/// The parameters, sample_count and interval_in_ms, are the parameters of the metric statistic you want to build
/// The parameters, parent_sample_count and parent_interval_in_ms, are the parameters of the resource's global statistic
/// If compliance passes, return Ok(()), if not returns specific error
pub fn check_validity_for_reuse_statistic(
    sample_count: u32,
    interval_in_ms: u32,
    parent_sample_count: u32,
    parent_interval_in_ms: u32,
) -> Result<()> {
    check_validity_for_statistic(sample_count, interval_in_ms, ILLEGAL_STATISTIC_PARAMS_ERROR)?;
    let bucket_length_in_ms = interval_in_ms / sample_count;

    check_validity_for_statistic(
        parent_sample_count,
        parent_interval_in_ms,
        ILLEGAL_GLOBAL_STATISTIC_PARAMS_ERROR,
    )?;
    let parent_bucket_length_in_ms = parent_interval_in_ms / parent_sample_count;

    //SlidingWindowMetric's intervalInMs is not divisible by BucketLeapArray's intervalInMs
    if parent_interval_in_ms % interval_in_ms != 0 {
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
            fn get_qps(&self) -> f64;
            fn get_previous_qps(&self) -> f64;
            fn get_sum(&self) -> i64;
            fn min_rt(&self) -> f64;
            fn avg_rt(&self) -> f64;
        }
        impl WriteStat for StatNode {
            fn add_count(&self, _metric_event: MetricEvent, _count: i64);
        }
        impl ConcurrencyStat for StatNode {
            fn current_concurrency(&self) -> i32;
            fn increase_concurrency(&self) ;
            fn decrease_concurrency(&self) ;
        }
        impl MetricItemRetriever for StatNode {
            fn metrics_on_condition(&self, predicate: TimePredicate) -> Vec<MetricItem>;
        }
        impl StatNode for StatNode {
            fn generate_read_stat(
                &self,
                sample_count: u32,
                interval_in_ms: u32,
            ) -> Result<Rc<RefCell<dyn ReadStat>>> ;
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
