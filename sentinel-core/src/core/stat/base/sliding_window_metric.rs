use super::{BucketLeapArray, BucketWrap, MetricBucket};
use crate::base::{
    check_validity_for_reuse_statistic, MetricEvent, MetricItem, ReadStat, TimePredicate,
    DEFAULT_STATISTIC_MAX_RT,
};
use crate::utils::curr_time_millis;
use crate::Result;
use std::cmp;
use std::collections::HashMap;
use std::sync::Arc;

// SlidingWindowMetric represents the sliding window metric wrapper,
// several of which might associated the same inner BucketLeapArray
// It does not store any data and is the wrapper of BucketLeapArray to adapt to different internal bucket
// SlidingWindowMetric is used for SentinelRules and BucketLeapArray is used for monitor
// BucketLeapArray is per resource, and SlidingWindowMetric support only read operation.
#[derive(Debug)]
pub struct SlidingWindowMetric {
    bucket_len_ms: u32,
    sample_count: u32,
    interval_ms: u32,
    inner: Arc<BucketLeapArray>,
}

#[allow(dead_code)]
// all the methods are getter
impl SlidingWindowMetric {
    pub fn new(sample_count: u32, interval_ms: u32, inner: Arc<BucketLeapArray>) -> Result<Self> {
        check_validity_for_reuse_statistic(
            sample_count,
            interval_ms,
            inner.sample_count(),
            inner.interval_ms(),
        )?;
        Ok(SlidingWindowMetric {
            bucket_len_ms: interval_ms / sample_count,
            sample_count,
            interval_ms,
            inner,
        })
    }

    pub fn interval_ms(&self) -> u32 {
        self.interval_ms
    }

    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub fn bucket_len_ms(&self) -> u32 {
        self.bucket_len_ms
    }

    /// Get the start time range of the bucket for the provided time.
    /// The actual time span is: [start, end + bucket_len_ms).
    /// Why? See `LeapArray::calculate_start_stamp()` and `LeapArray::get_valid_values_conditional()`
    pub(crate) fn bucket_start_range(&self, t_ms: u64) -> (u64, u64) {
        let end = self.inner.calculate_start_stamp(t_ms);
        let start = end - self.interval_ms as u64 + self.inner.bucket_len_ms() as u64;
        (start, end)
    }

    pub(crate) fn satisfied_buckets(&self, now: u64) -> Vec<Arc<BucketWrap<MetricBucket>>> {
        let (start, end) = self.bucket_start_range(now);
        self.inner
            .get_valid_values_conditional(now, &move |curr: u64| start <= curr && curr <= end)
    }

    pub fn interval_s(&self) -> f64 {
        self.interval_ms as f64 / 1000.0
    }

    pub fn sum_with_time(&self, now: u64, event: MetricEvent) -> u64 {
        let buckets = self.satisfied_buckets(now);
        let mut res = 0;
        for b in buckets {
            res += b.value().get(event);
        }
        res
    }

    pub fn qps_with_time(&self, now: u64, event: MetricEvent) -> f64 {
        self.sum_with_time(now, event) as f64 / self.interval_s()
    }

    pub fn max_of_single_bucket(&self, event: MetricEvent) -> u64 {
        let buckets = self.satisfied_buckets(curr_time_millis());
        let mut res = 0;
        for b in buckets {
            res = cmp::max(res, b.value().get(event));
        }
        res
    }

    pub fn max_concurrency(&self) -> u32 {
        let buckets = self.satisfied_buckets(curr_time_millis());
        let mut res = 0;
        for b in buckets {
            res = cmp::max(res, b.value().max_concurrency());
        }
        res
    }

    /// second_metrics_on_condition aggregates metric items by second on condition that
    /// the startTime of the statistic buckets satisfies the time predicate.
    pub fn second_metrics_on_condition(&self, condition: &TimePredicate) -> Vec<MetricItem> {
        let buckets = self
            .inner
            .get_valid_values_conditional(curr_time_millis(), condition);
        // Aggregate second-level MetricItem (only for stable metrics)
        let mut buckets_map = HashMap::<u64, Vec<Arc<BucketWrap<MetricBucket>>>>::new();
        for b in buckets {
            let start_stamp = b.start_stamp();
            // eliminates differences in millisecond-level
            let second_start = start_stamp - start_stamp % 1000;
            buckets_map
                .entry(second_start)
                .or_insert_with(Vec::new)
                .push(b);
        }
        let mut res = Vec::new();
        for (timestamp, b) in buckets_map {
            if !b.is_empty() {
                res.push(self.metric_item_from_buckets(timestamp, b));
            }
        }
        res
    }

    pub(crate) fn metric_item_from_buckets(
        &self,
        timestamp: u64,
        buckets: Vec<Arc<BucketWrap<MetricBucket>>>,
    ) -> MetricItem {
        let mut metric_item = MetricItem::default();
        let mut all_rt = 0;
        metric_item.timestamp = timestamp;
        for bucket in buckets {
            let b = bucket.value();
            metric_item.pass_qps += b.get(MetricEvent::Pass);
            metric_item.block_qps += b.get(MetricEvent::Block);
            metric_item.error_qps += b.get(MetricEvent::Error);
            metric_item.complete_qps += b.get(MetricEvent::Complete);
            metric_item.concurrency = cmp::max(b.max_concurrency(), metric_item.concurrency);
            all_rt += b.get(MetricEvent::Rt);
        }
        if metric_item.complete_qps > 0 {
            metric_item.avg_rt = all_rt / metric_item.complete_qps;
        } else {
            metric_item.avg_rt = all_rt;
        }
        metric_item
    }

    pub(crate) fn metric_item_from_bucket(
        &self,
        bucket: Arc<BucketWrap<MetricBucket>>,
    ) -> MetricItem {
        let timestamp = bucket.start_stamp();
        let bucket = bucket.value();
        let complete_qps = bucket.get(MetricEvent::Complete);
        let avg_rt = if complete_qps > 0 {
            bucket.get(MetricEvent::Rt) / complete_qps
        } else {
            bucket.get(MetricEvent::Rt)
        };
        MetricItem {
            timestamp,
            pass_qps: bucket.get(MetricEvent::Pass),
            block_qps: bucket.get(MetricEvent::Block),
            complete_qps,
            error_qps: bucket.get(MetricEvent::Error),
            avg_rt,
            ..MetricItem::default()
        }
    }
}

impl ReadStat for SlidingWindowMetric {
    fn qps(&self, event: MetricEvent) -> f64 {
        self.qps_with_time(curr_time_millis(), event)
    }

    fn qps_previous(&self, event: MetricEvent) -> f64 {
        self.qps_with_time(curr_time_millis() - self.bucket_len_ms as u64, event)
    }

    fn sum(&self, event: MetricEvent) -> u64 {
        self.sum_with_time(curr_time_millis(), event)
    }

    fn avg_rt(&self) -> f64 {
        let completed = self.sum(MetricEvent::Complete);
        if completed == 0 {
            0f64
        } else {
            self.sum(MetricEvent::Rt) as f64 / completed as f64
        }
    }

    fn min_rt(&self) -> f64 {
        let buckets = self.satisfied_buckets(curr_time_millis());
        let mut res = DEFAULT_STATISTIC_MAX_RT;
        for b in buckets {
            res = cmp::min(res, b.value().min_rt());
        }
        res as f64
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::stat::WriteStat;
    use std::thread;
    const SAMPLE_COUNT: u32 = 20;
    const BUCKET_LEN_MS: u32 = 500; // 500 ms
    const INTERVAL_MS: u32 = BUCKET_LEN_MS * SAMPLE_COUNT; // 10 s

    #[test]
    fn new() {
        assert!(SlidingWindowMetric::new(
            4,
            2000,
            Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap())
        )
        .is_ok());
        assert!(SlidingWindowMetric::new(
            0,
            0,
            Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap())
        )
        .is_err());
        assert!(SlidingWindowMetric::new(
            4,
            2001,
            Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap())
        )
        .is_err());
        assert!(SlidingWindowMetric::new(
            2,
            2002,
            Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap())
        )
        .is_err());
        assert!(SlidingWindowMetric::new(
            4,
            200000,
            Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap())
        )
        .is_err());
    }

    #[test]
    fn start_range() {
        struct Testcase {
            sample_count: u32,
            interval_ms: u32,
            inner_sample_count: u32,
            inner_interval_ms: u32,
            now: u64,
            wanted_start: u64,
            wanted_end: u64,
        }
        let testcases = [
            Testcase {
                sample_count: 4,
                interval_ms: 2000,
                inner_sample_count: 20,
                inner_interval_ms: 10000,
                // array start time:1578416550000
                // bucket start time:1578416556500
                now: 1578416556900,
                wanted_start: 1578416555000,
                wanted_end: 1578416556500,
            },
            Testcase {
                sample_count: 2,
                interval_ms: 1000,
                inner_sample_count: 20,
                inner_interval_ms: 10000,
                // array start time:1578416550000
                // bucket start time:1578416556500
                now: 1578416556900,
                wanted_start: 1578416556000,
                wanted_end: 1578416556500,
            },
            Testcase {
                sample_count: 1,
                interval_ms: 2000,
                inner_sample_count: 10,
                inner_interval_ms: 10000,
                // array start time:1578416550000
                // bucket start time:1578416556500
                now: 1578416556900,
                wanted_start: 1578416555000,
                wanted_end: 1578416556000,
            },
            Testcase {
                sample_count: 1,
                interval_ms: 10000,
                inner_sample_count: 10,
                inner_interval_ms: 20000,
                // array start time:1578416550000
                // bucket start time:1578416556500
                now: 1578416556900,
                wanted_start: 1578416548000,
                wanted_end: 1578416556000,
            },
            Testcase {
                sample_count: 2,
                interval_ms: 1000,
                inner_sample_count: 20,
                inner_interval_ms: 10000,
                // array start time:1578416550000
                // bucket start time:1578416556500
                now: 1578416556500,
                wanted_start: 1578416556000,
                wanted_end: 1578416556500,
            },
        ];

        for tc in testcases {
            let swm = SlidingWindowMetric::new(
                tc.sample_count,
                tc.interval_ms,
                Arc::new(
                    BucketLeapArray::new(tc.inner_sample_count, tc.inner_interval_ms).unwrap(),
                ),
            )
            .unwrap();
            let (start, end) = swm.bucket_start_range(tc.now);
            assert_eq!(tc.wanted_start, start);
            assert_eq!(tc.wanted_end, end);
        }
    }

    #[test]
    fn sum_with_time() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms, now) = (2, 2000, 1678416556599);
        let mut handles = Vec::new();
        for _ in 0..500 {
            handles.push(thread::spawn({
                let arr = arr.clone();
                move || {
                    arr.add_count_with_time(now, MetricEvent::Pass, 1).unwrap();
                }
            }))
        }
        for i in 0..interval_ms as u64 {
            handles.push(thread::spawn({
                let arr = arr.clone();
                move || {
                    arr.add_count_with_time(now - 100 - i, MetricEvent::Pass, 1)
                        .unwrap();
                }
            }))
        }
        for h in handles {
            h.join().unwrap();
        }
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr).unwrap();
        assert_eq!(swm.sum_with_time(now, MetricEvent::Pass), 2000);
    }

    #[test]
    fn max_of_single_bucket() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms) = (2, 2000);
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr.clone()).unwrap();
        arr.add_count(MetricEvent::Pass, 100);
        assert_eq!(swm.max_of_single_bucket(MetricEvent::Pass), 100);
    }

    #[test]
    fn min_rt() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms) = (2, 2000);
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr).unwrap();
        assert!((swm.min_rt() - DEFAULT_STATISTIC_MAX_RT as f64).abs() < f64::EPSILON);
    }

    #[test]
    fn max_concurrency() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms) = (4, 2000);
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr.clone()).unwrap();
        arr.update_concurrency(1);
        arr.update_concurrency(3);
        arr.update_concurrency(2);
        assert_eq!(swm.max_concurrency(), 3);
    }

    #[test]
    fn avg_rt() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms) = (4, 2000);
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr.clone()).unwrap();
        arr.add_count(MetricEvent::Rt, 100);
        arr.add_count(MetricEvent::Complete, 100);
        assert!((swm.avg_rt() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn metric_item_from_buckets() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms, now) = (4, 2000, curr_time_millis());
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr.clone()).unwrap();
        arr.add_count(MetricEvent::Pass, 100);
        let item = swm.metric_item_from_buckets(now, swm.satisfied_buckets(now));
        assert_eq!(item.pass_qps, 100);
    }

    #[test]
    fn metric_item_from_bucket() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms, now) = (4, 2000, curr_time_millis());
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr).unwrap();
        let bucket = Arc::new(BucketWrap::<MetricBucket>::new(now));
        bucket.value().add_count(MetricEvent::Pass, 100);
        let item = swm.metric_item_from_bucket(bucket);
        assert_eq!(item.pass_qps, 100);
    }

    #[test]
    fn second_metrics_on_condition() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let (sample_count, interval_ms, now) = (4, 2000, curr_time_millis());
        let swm = SlidingWindowMetric::new(sample_count, interval_ms, arr.clone()).unwrap();
        arr.add_count_with_time(now, MetricEvent::Pass, 100)
            .unwrap();
        arr.add_count_with_time(now - 1000, MetricEvent::Pass, 100)
            .unwrap();
        let (start, end) = swm.bucket_start_range(now);
        let item = swm.second_metrics_on_condition(&move |ts| start <= ts && ts <= end);
        assert_eq!(item.len(), 2);
    }
}
