use super::MetricTrait;
use crate::base::TimePredicate;
use crate::utils::curr_time_millis;
use crate::{Error, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const DEFAULT_TIME: u64 = 0;

/// BucketWrap represent a slot to record metrics
/// The metric itself should be atomic
/// The length of BucketWrap could be seen in LeapArray.
/// The scope of time is [start_stamp, start_stamp+bucket_length)
#[derive(Debug, Default)]
pub struct BucketWrap<T: MetricTrait> {
    // The start timestamp of this statistic bucket wrapper.
    start_stamp: AtomicU64,
    // The actual data structure to record the metrics (e.g. MetricBucket).
    value: T,
}

impl<T: MetricTrait> BucketWrap<T> {
    pub fn new(start_stamp: u64) -> Self {
        BucketWrap {
            start_stamp: AtomicU64::new(start_stamp),
            value: T::default(),
        }
    }

    pub fn start_stamp(&self) -> u64 {
        self.start_stamp.load(Ordering::SeqCst)
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn reset_start_stamp(&self, start_stamp: u64) {
        self.start_stamp.store(start_stamp, Ordering::SeqCst);
    }

    pub fn reset_value(&self) {
        self.value.reset();
    }

    pub fn is_time_in_bucket(&self, now: u64, bucket_len_ms: u32) -> bool {
        let start = self.start_stamp.load(Ordering::SeqCst);
        start <= now && now < start + (bucket_len_ms as u64)
    }

    pub fn is_deprecated(&self, now: u64, interval: u64) -> bool {
        let start = self.start_stamp.load(Ordering::SeqCst);
        now > start && now - start > interval
    }
}

/// The BucketWrap leap array,
/// it treats the inner array as a ring
/// sampleCount represent the number of BucketWrap
/// intervalInMs represent the interval of LeapArray.
/// Currently, the race condition resolving relies on inner atomatic metric, e.g., MetricBucket T
/// For example, bucket_len_ms is 200ms, interval_ms is 1000ms, so sample_count is 5.
#[derive(Debug)]
pub struct LeapArray<T: MetricTrait> {
    bucket_len_ms: u32,
    sample_count: u32,
    interval_ms: u32,
    pub(crate) array: Vec<Arc<BucketWrap<T>>>, // todo: remove the Arc here?
    // todo: benchmark, compare with a single lock for the whole array, one RWLock/Mutex wrapping array
    mutex: Vec<Mutex<bool>>,
}

impl<T: MetricTrait> LeapArray<T> {
    pub fn new(sample_count: u32, interval_ms: u32) -> Result<Self> {
        if sample_count == 0 || interval_ms % sample_count != 0 {
            return Err(Error::msg(
                "Invalid sample count or interval_ms. Time span needs to be evenly divided",
            ));
        }
        let mut array = Vec::with_capacity(sample_count as usize);
        let mut mutex = Vec::with_capacity(sample_count as usize);
        for _ in 0..sample_count {
            array.push(Arc::new(BucketWrap::default()));
            mutex.push(Mutex::new(false));
        }
        // set start time of the buckets in LeapArray
        Ok(LeapArray {
            bucket_len_ms: interval_ms / sample_count,
            sample_count,
            interval_ms,
            array,
            mutex,
        })
    }

    pub fn bucket_len_ms(&self) -> u32 {
        self.bucket_len_ms
    }

    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub fn interval_ms(&self) -> u32 {
        self.interval_ms
    }

    // todo: bechmark, use default trait to create a new object and reset completely,
    // or reset fields one by one by an assoiciated type?
    pub fn reset_bucket(&self, idx: usize, start_stamp: u64) {
        self.array[idx].reset_start_stamp(start_stamp);
        self.array[idx].reset_value();
    }

    pub fn current_bucket(&self) -> Result<Arc<BucketWrap<T>>> {
        self.get_bucket_of_time(curr_time_millis())
    }

    pub fn get_bucket_of_time(&self, now: u64) -> Result<Arc<BucketWrap<T>>> {
        let idx = self.time2idx(now) as usize;
        let target_start = self.calculate_start_stamp(now);
        /*
        Get bucket item at given time from the array.
        - (1) Bucket is absent, then just create a new bucket and CAS update to circular array.
        - (2) Bucket is up-to-date, then just return the bucket.
        - (3) Bucket is deprecated, then reset current bucket and clean all deprecated buckets.
        */
        let bucket = self.array[idx].clone(); // nonexpect
        loop {
            if bucket.start_stamp() == DEFAULT_TIME {
                /*
                     B0       B1      B2    NULL      B4
                ||_______|_______|_______|_______|_______||___
                200     400     600     800     1000    1200  timestamp
                                            ^
                                         time=888
                           bucket is empty, so create new and update
                If the old bucket is absent, then we update the timestamp BucketWrapper in circular array */
                bucket.reset_start_stamp(target_start);
                return Ok(Arc::clone(&bucket));
            } else if bucket.start_stamp() == target_start {
                /*
                    B0       B1      B2     B3      B4
                ||_______|_______|_______|_______|_______||___
                200     400     600     800     1000    1200  timestamp
                                            ^
                                         time=888
                           startTime of Bucket 3: 800, so it's up-to-date
                If current {@code windowStart} is equal to the start timestamp of old bucket,
                that means the time is within the bucket, so directly return the bucket.
                 */
                return Ok(Arc::clone(&bucket));
            } else if target_start > bucket.start_stamp() {
                /*
                  (old)
                            B0       B1      B2    NULL      B4
                |_______||_______|_______|_______|_______|_______||___
                ...    1200     1400    1600    1800    2000    2200  timestamp
                                             ^
                                          time=1676
                         startTime of Bucket 2: 400, deprecated, should be reset
                If the start timestamp of old bucket is behind provided time, that means
                the bucket is deprecated. We have to reset the bucket to current target_start.
                Note that the reset and clean-up operations are hard to be atomic,
                so we need a update lock to guarantee the correctness of bucket update.
                The update lock is conditional (tiny scope) and will take effect only when
                bucket is deprecated, so in most cases it won't lead to performance loss.
                 */
                if self.mutex[idx].try_lock().is_ok() {
                    self.reset_bucket(idx, target_start);
                    return Ok(Arc::clone(&self.array[idx]));
                } else {
                    // during sleeping, other thread may have reset the bucket
                    std::thread::yield_now();
                }
            } else {
                return Err(Error::msg("invalid time stamp, cannot find bucket"));
            }
        }
    }

    /// Get the previous bucket item for current timestamp.
    pub fn get_previous_bucket(&self) -> Result<Arc<BucketWrap<T>>> {
        let previous = curr_time_millis() - (self.bucket_len_ms as u64);
        let idx = self.time2idx(previous) as usize;
        let bucket = self.array[idx].clone(); // nonexpect
        if bucket.is_deprecated(curr_time_millis(), self.interval_ms as u64) {
            return Err(Error::msg("previous bucket has been deprecated"));
        }
        if bucket.start_stamp() + (self.bucket_len_ms as u64) < previous {
            return Err(Error::msg("the timestamp of returnning bucket is wrong"));
        }
        Ok(bucket)
    }

    /// compute the start timestamp of current bucket
    pub(crate) fn calculate_start_stamp(&self, now: u64) -> u64 {
        now - now % (self.bucket_len_ms as u64)
    }

    pub(crate) fn time2idx(&self, now: u64) -> u64 {
        let idx = now / (self.bucket_len_ms as u64);
        idx % (self.sample_count as u64)
    }

    pub fn valid_array(&self) -> Vec<Arc<BucketWrap<T>>> {
        let mut res = Vec::new();
        for bucket in &self.array {
            if !bucket.is_deprecated(curr_time_millis(), self.interval_ms as u64) {
                res.push(bucket.clone());
            }
        }
        res
    }

    pub fn get_bucket_value(&self, now: u64) -> Result<&T> {
        let idx = self.time2idx(now) as usize;
        let bucket = &self.array[idx]; // nonexpect
        if bucket.is_time_in_bucket(now, self.bucket_len_ms) {
            Ok(bucket.value())
        } else {
            Err(Error::msg("invalid time, cannot get value in the bucket"))
        }
    }

    pub fn get_current_values(&self) -> Vec<Arc<BucketWrap<T>>> {
        self.get_valid_values(curr_time_millis())
    }

    ///  Get all BucketWrap between [current time - leap array interval, current time]
    pub fn get_valid_values(&self, now: u64) -> Vec<Arc<BucketWrap<T>>> {
        self.get_valid_values_conditional(now, &|_| true)
    }

    pub fn get_valid_values_conditional(
        &self,
        now: u64,
        condition: &TimePredicate,
    ) -> Vec<Arc<BucketWrap<T>>> {
        let mut res = Vec::new();
        for bucket in &self.array {
            if !bucket.is_deprecated(now, self.interval_ms as u64)
                && condition(bucket.start_stamp())
            {
                res.push(bucket.clone());
            }
        }
        res
    }

    #[cfg(test)]
    pub(self) fn get_valid_head(&self) -> Result<Arc<BucketWrap<T>>> {
        let idx = self.time2idx(curr_time_millis() + (self.bucket_len_ms as u64)) as usize;
        let bucket = self.array[idx].clone();
        if bucket.is_deprecated(curr_time_millis(), self.interval_ms as u64) {
            Err(Error::msg("Cannot get a valid head"))
        } else {
            Ok(bucket)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::{thread, time};

    const SAMPLE_COUNT: u32 = 20;
    const BUCKET_LEN_MS: u32 = 500; // 500 ms
    const INTERVAL_MS: u32 = BUCKET_LEN_MS * SAMPLE_COUNT; // 10 s

    impl MetricTrait for AtomicU64 {
        fn reset(&self) {
            self.store(0, Ordering::SeqCst);
        }
    }
    type LeapArrayAtomicU64 = LeapArray<AtomicU64>;

    #[test]
    fn time_idx() {
        let arr = LeapArrayAtomicU64::new(SAMPLE_COUNT, INTERVAL_MS).unwrap();
        assert_eq!(arr.time2idx(1576296044907), 9);
        assert_eq!(arr.calculate_start_stamp(1576296044907), 1576296044500);
    }

    #[test]
    fn start_time() {
        let mut arr = LeapArrayAtomicU64::new(SAMPLE_COUNT, INTERVAL_MS).unwrap();
        let now = 1596199310000;
        let bucket = arr.get_bucket_of_time(now + 801).unwrap();
        assert_eq!(bucket.start_stamp(), now + 500);
        assert!(Arc::ptr_eq(&bucket, arr.array.get(1).unwrap()));
    }

    #[test]
    fn deprecated() {
        let now = 1576296044907;
        let bucket = BucketWrap::<AtomicU64>::new(1576296004907);
        assert!(bucket.is_deprecated(now, INTERVAL_MS as u64));
    }

    #[test]
    #[ignore]
    fn valid_head() {
        let sample_count = 10;
        let interval_ms = 1000;
        let bucket_len_ms = (interval_ms / sample_count) as u64;
        let mut arr = LeapArrayAtomicU64::new(sample_count, interval_ms).unwrap();

        let window = time::Duration::from_millis(bucket_len_ms);
        for i in 1..=(sample_count as u64) {
            thread::sleep(window);
            arr.current_bucket()
                .unwrap()
                .value()
                .store(i, Ordering::SeqCst);
        }
        thread::sleep(window);
        let head = arr.get_valid_head().unwrap();
        assert_eq!(2, head.value().load(Ordering::SeqCst));
    }
}
