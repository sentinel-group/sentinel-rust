use crate::base::{MetricEvent, DEFAULT_STATISTIC_MAX_RT};
use enum_map::EnumMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// use atomic types to ensure metric's internal mutability
/// otherwise, exclusive Mutex would be necessary on the LeapArray Arc among threads
/// todo: or use AtomicPtr?
pub trait MetricTrait: fmt::Debug + Default + Send + Sync {
    fn reset(&self);
}

/// MetricBucket represents the entity to record metrics per minimum time unit (i.e. the bucket time span).
/// Note that all operations of the MetricBucket are required to be thread-safe.
#[derive(Debug)]
pub struct MetricBucket {
    // EnumMap should work as fast as arrays
    counter: EnumMap<MetricEvent, AtomicU64>,
    min_rt: AtomicU64,
    max_concurrency: AtomicU32,
}

impl MetricTrait for MetricBucket {
    fn reset(&self) {
        for (_, item) in &self.counter {
            item.store(0, Ordering::SeqCst);
        }
        self.min_rt
            .store(DEFAULT_STATISTIC_MAX_RT, Ordering::SeqCst);
        self.max_concurrency.store(0, Ordering::SeqCst);
    }
}

impl Default for MetricBucket {
    fn default() -> Self {
        MetricBucket {
            counter: EnumMap::default(),
            min_rt: AtomicU64::new(DEFAULT_STATISTIC_MAX_RT),
            max_concurrency: AtomicU32::new(0),
        }
    }
}

impl MetricBucket {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add statistic count for the given metric event.
    pub fn add(&self, event: MetricEvent, count: u64) {
        match event {
            MetricEvent::Rt => self.add_rt(count),
            _ => self.add_count(event, count),
        }
    }

    pub fn add_count(&self, event: MetricEvent, count: u64) {
        self.counter[event].fetch_add(count, Ordering::SeqCst);
    }

    pub fn add_rt(&self, round_trip: u64) {
        self.add_count(MetricEvent::Rt, round_trip);
        if round_trip < self.min_rt.load(Ordering::SeqCst) {
            // Might not be accurate here.
            self.min_rt.store(round_trip, Ordering::SeqCst);
        }
    }

    /// Get current statistic count of the given metric event.
    pub fn get(&self, event: MetricEvent) -> u64 {
        self.counter[event].load(Ordering::SeqCst)
    }

    pub fn min_rt(&self) -> u64 {
        self.min_rt.load(Ordering::SeqCst)
    }
    pub fn update_concurrency(&self, concurrency: u32) {
        if concurrency > self.max_concurrency.load(Ordering::SeqCst) {
            // Might not be accurate here.
            self.max_concurrency.store(concurrency, Ordering::SeqCst);
        }
    }
    pub fn max_concurrency(&self) -> u32 {
        self.max_concurrency.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;
    use std::thread::spawn;

    #[test]
    fn single() {
        let mb = MetricBucket::new();
        for i in 0..120 {
            match i % 6 {
                0 => mb.add(MetricEvent::Pass, 1),
                1 => mb.add(MetricEvent::Block, 1),
                2 => mb.add(MetricEvent::Complete, 1),
                3 => mb.add(MetricEvent::Error, 1),
                4 => mb.add_rt(100),
                5 => mb.update_concurrency(i),
                _ => {}
            }
        }
        assert_eq!(
            mb.get(MetricEvent::Pass),
            20,
            "unexpect count MetricEventPass"
        );
        assert_eq!(
            mb.get(MetricEvent::Block),
            20,
            "unexpect count MetricEventBlock"
        );
        assert_eq!(
            mb.get(MetricEvent::Complete),
            20,
            "unexpect count MetricEventComplete"
        );
        assert_eq!(
            mb.get(MetricEvent::Error),
            20,
            "unexpect count MetricEventError"
        );
        assert_eq!(
            mb.get(MetricEvent::Rt),
            2000,
            "unexpect count MetricEventRt"
        );
        assert_eq!(
            mb.max_concurrency(),
            119,
            "unexpect count MetricEventConcurrency"
        );
    }

    #[test]
    fn concurrent() {
        let mb_arc = Arc::new(MetricBucket::new());
        let mut handles = Vec::new();
        for _ in 0..1000 {
            let mb = mb_arc.clone();
            handles.push(spawn(move || {
                mb.add(MetricEvent::Pass, 1);
            }))
        }
        for _ in 0..1000 {
            let mb = mb_arc.clone();
            handles.push(spawn(move || {
                mb.add(MetricEvent::Block, 2);
            }))
        }
        for _ in 0..1000 {
            let mb = mb_arc.clone();
            handles.push(spawn(move || {
                mb.add(MetricEvent::Complete, 3);
            }))
        }
        for _ in 0..1000 {
            let mb = mb_arc.clone();
            handles.push(spawn(move || {
                mb.add(MetricEvent::Error, 4);
            }))
        }
        for i in 0..1000 {
            let mb = mb_arc.clone();
            handles.push(spawn(move || {
                mb.add(MetricEvent::Rt, i);
            }))
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            mb_arc.get(MetricEvent::Pass),
            1000,
            "unexpect count MetricEventPass"
        );
        assert_eq!(
            mb_arc.get(MetricEvent::Block),
            2000,
            "unexpect count MetricEventBlock"
        );
        assert_eq!(
            mb_arc.get(MetricEvent::Complete),
            3000,
            "unexpect count MetricEventComplete"
        );
        assert_eq!(
            mb_arc.get(MetricEvent::Error),
            4000,
            "unexpect count MetricEventError"
        );
        assert_eq!(
            mb_arc.get(MetricEvent::Rt),
            499_500,
            "unexpect count MetricEventRt"
        );
    }

    #[test]
    fn reset() {
        let mb = Arc::new(MetricBucket::new());
        mb.add_rt(100);
        mb.reset();
        assert_eq!(mb.min_rt(), DEFAULT_STATISTIC_MAX_RT);
        assert_eq!(mb.max_concurrency(), 0);
    }
}
