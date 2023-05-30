use super::{LeapArray, MetricBucket};
use crate::base::{MetricEvent, WriteStat, DEFAULT_STATISTIC_MAX_RT};
use crate::utils::curr_time_millis;
use crate::Result;
use std::cmp;

/// a specialization of `LeapArray<T>` with `MetricBucket`
pub type BucketLeapArray = LeapArray<MetricBucket>;

impl WriteStat for BucketLeapArray {
    fn add_count(&self, event: MetricEvent, count: u64) {
        self.add_count_with_time(curr_time_millis(), event, count)
            .unwrap();
    }

    fn update_concurrency(&self, concurrency: u32) {
        self.update_concurrency_with_time(curr_time_millis(), concurrency)
            .unwrap();
    }
}

impl BucketLeapArray {
    pub fn add_count_with_time(&self, now: u64, event: MetricEvent, count: u64) -> Result<()> {
        let bucket = self.get_bucket_of_time(now)?;
        bucket.value().add(event, count);
        Ok(())
    }

    pub fn update_concurrency_with_time(&self, now: u64, concurrency: u32) -> Result<()> {
        let bucket = self.get_bucket_of_time(now)?;
        bucket.value().update_concurrency(concurrency);
        Ok(())
    }

    pub fn count(&self, event: MetricEvent) -> u64 {
        self.count_with_time(curr_time_millis(), event)
    }

    pub fn count_with_time(&self, now: u64, event: MetricEvent) -> u64 {
        let mut res = 0;
        let buckets = self.get_valid_values(now);
        for b in buckets {
            res += b.value().get(event);
        }
        res
    }

    pub fn min_rt(&self) -> u64 {
        let mut res = DEFAULT_STATISTIC_MAX_RT;
        let buckets = self.get_current_values();
        for b in buckets {
            res = cmp::min(res, b.value().min_rt());
        }
        res
    }

    pub fn max_concurrency(&self) -> u32 {
        let mut res = 0;
        let buckets = self.get_current_values();
        for b in buckets {
            res = cmp::max(res, b.value().max_concurrency());
        }
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::thread;

    const SAMPLE_COUNT: u32 = 20;
    const BUCKET_LEN_MS: u32 = 500; // 500 ms
    const INTERVAL_MS: u32 = BUCKET_LEN_MS * SAMPLE_COUNT; // 10 s

    #[test]
    fn reset_bucket() {
        let arr = BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap();
        let idx = 19;
        arr.array[idx].value().add(MetricEvent::Block, 100);
        let want_start_time = curr_time_millis() + 1000;
        arr.reset_bucket(idx, want_start_time);
        assert_eq!(arr.array[idx].value().get(MetricEvent::Block), 0);
        assert_eq!(arr.array[idx].start_stamp(), want_start_time);
    }

    #[test]
    fn update_concurrency() {
        let arr = BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap();
        assert_eq!(arr.max_concurrency(), 0);
        arr.update_concurrency(1);
        arr.update_concurrency(2);
        arr.update_concurrency(3);
        assert_eq!(arr.max_concurrency(), 3);
    }

    #[test]
    fn add_count() {
        let arr = BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap();
        arr.add_count(MetricEvent::Pass, 3);
        arr.add_count(MetricEvent::Block, 1);
        assert_eq!(arr.count(MetricEvent::Pass), 3);
        assert_eq!(arr.count(MetricEvent::Block), 1);
        assert_eq!(arr.count(MetricEvent::Complete), 0);
    }

    #[test]
    fn min_rt() {
        let arr = BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap();
        assert_eq!(arr.min_rt(), DEFAULT_STATISTIC_MAX_RT);
        arr.add_count(MetricEvent::Rt, 100);
        assert_eq!(arr.min_rt(), 100);
    }

    #[test]
    fn concurrent() {
        let arr = Arc::new(BucketLeapArray::new(SAMPLE_COUNT, INTERVAL_MS).unwrap());
        let now = 1976296040000u64;
        let mut t = now;
        while t < now + INTERVAL_MS as u64 {
            arr.add_count_with_time(t, MetricEvent::Pass, 1).unwrap();
            arr.add_count_with_time(t, MetricEvent::Block, 1).unwrap();
            arr.add_count_with_time(t, MetricEvent::Error, 1).unwrap();
            arr.add_count_with_time(t, MetricEvent::Complete, 1)
                .unwrap();
            arr.add_count_with_time(t, MetricEvent::Rt, 10).unwrap();
            t += BUCKET_LEN_MS as u64;
        }
        for b in arr.get_valid_values(now + 9999) {
            assert_eq!(b.value().get(MetricEvent::Pass), 1);
            assert_eq!(b.value().get(MetricEvent::Block), 1);
            assert_eq!(b.value().get(MetricEvent::Error), 1);
            assert_eq!(b.value().get(MetricEvent::Complete), 1);
            assert_eq!(b.value().get(MetricEvent::Rt), 10);
        }
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Pass), 20);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Block), 20);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Complete), 20);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Error), 20);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Rt), 200);

        let counter = Arc::new(AtomicU64::new(0));
        let mut handles = Vec::new();
        for _ in 0..3000 {
            handles.push(thread::spawn({
                let arr = arr.clone();
                let counter = counter.clone();
                move || {
                    let timestamp = rand::random::<u64>() % INTERVAL_MS as u64;
                    arr.add_count_with_time(now + timestamp, MetricEvent::Pass, 1)
                        .unwrap();
                    arr.add_count_with_time(now + timestamp, MetricEvent::Block, 1)
                        .unwrap();
                    arr.add_count_with_time(now + timestamp, MetricEvent::Complete, 1)
                        .unwrap();
                    arr.add_count_with_time(now + timestamp, MetricEvent::Error, 1)
                        .unwrap();
                    arr.add_count_with_time(now + timestamp, MetricEvent::Rt, 10)
                        .unwrap();
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 3000);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Pass), 3020);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Block), 3020);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Complete), 3020);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Error), 3020);
        assert_eq!(arr.count_with_time(now + 9999, MetricEvent::Rt), 30200);
    }
}
