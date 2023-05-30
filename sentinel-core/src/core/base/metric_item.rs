//! Metric Item
//!

use super::{ResourceType, TimePredicate};
use crate::utils::format_time_millis;
use crate::{Error, Result};
use std::fmt;

pub const METRIC_PART_SEPARATOR: &str = "|";
pub const METRIC_EMPTY_STRING_ERROR: &str = "invalid metric line: empty string";
pub const METRIC_INVALID_FORMAT_ERROR: &str = "invalid metric line: invalid format";

/// MetricItem represents the data of metric log per line.
#[derive(Debug, Clone, Default)]
pub struct MetricItem {
    pub(crate) resource: String,
    pub(crate) resource_type: ResourceType,
    pub(crate) timestamp: u64,
    pub(crate) pass_qps: u64,
    pub(crate) block_qps: u64,
    pub(crate) complete_qps: u64,
    pub(crate) error_qps: u64,
    pub(crate) avg_rt: u64,
    pub(crate) occupied_pass_qps: u64,
    pub(crate) concurrency: u32,
}

impl fmt::Display for MetricItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let time_str = format_time_millis(self.timestamp);
        let final_name = self.resource.replace(METRIC_PART_SEPARATOR, "_");
        write!(
            f,
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.timestamp,
            time_str,
            final_name,
            self.pass_qps,
            self.block_qps,
            self.complete_qps,
            self.error_qps,
            self.avg_rt,
            self.occupied_pass_qps,
            self.concurrency,
            self.resource_type as u8
        )
    }
}

impl MetricItem {
    /// cannot use String trait, since conversion may fail
    pub fn from_string(line: &str) -> Result<Self> {
        if line.is_empty() {
            return Err(Error::msg(METRIC_EMPTY_STRING_ERROR));
        }
        let arr: Vec<&str> = line.split(METRIC_PART_SEPARATOR).collect();
        if arr.len() < 8 {
            return Err(Error::msg(METRIC_INVALID_FORMAT_ERROR));
        }
        let mut item = MetricItem {
            timestamp: arr[0].parse::<u64>()?,
            resource: arr[2].into(),
            pass_qps: arr[3].parse::<u64>()?,
            block_qps: arr[4].parse::<u64>()?,
            complete_qps: arr[5].parse::<u64>()?,
            error_qps: arr[6].parse::<u64>()?,
            avg_rt: arr[7].parse::<u64>()?,
            ..Default::default()
        };
        if arr.len() >= 9 {
            item.occupied_pass_qps = arr[8].parse::<u64>()?;
            if arr.len() >= 10 {
                item.concurrency = arr[9].parse::<u32>()?;
                if arr.len() >= 11 {
                    item.resource_type = arr[10].parse::<u8>()?.into();
                }
            }
        }
        Ok(item)
    }
}

pub trait MetricItemRetriever: Send + Sync {
    fn metrics_on_condition(&self, predicate: &TimePredicate) -> Vec<MetricItem>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn legal() {
        let metric_item = "1564382218000|2019-07-29 14:36:58|/foo/*|4|9|3|0|25|0|2|1";
        let metric_item = MetricItem::from_string(metric_item).unwrap();
        assert_eq!(1564382218000u64, metric_item.timestamp);
        assert_eq!(4u64, metric_item.pass_qps);
        assert_eq!(9u64, metric_item.block_qps);
        assert_eq!(3u64, metric_item.complete_qps);
        assert_eq!(0u64, metric_item.error_qps);
        assert_eq!(25u64, metric_item.avg_rt);
        assert_eq!("/foo/*", metric_item.resource);
        assert_eq!(1u8, metric_item.resource_type as u8);
    }

    #[test]
    #[should_panic(expected = "invalid metric line: empty string")] //METRIC_EMPTY_STRING_ERROR
    fn illegal1() {
        let metric_item = "";
        MetricItem::from_string(metric_item).unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid metric line: invalid format")] //METRIC_INVALID_FORMAT_ERROR
    fn illegal2() {
        let metric_item = "1564382218000|2019-07-29 14:36:58|/foo/*|4";
        MetricItem::from_string(metric_item).unwrap();
    }

    #[test]
    #[should_panic(expected = "invalid digit found in string")]
    fn illegal3() {
        let metric_item = "1564382218000|2019-07-29 14:36:58|/foo/*|4|-3|3|0|25|0|2|1";
        MetricItem::from_string(metric_item).unwrap();
    }
}
