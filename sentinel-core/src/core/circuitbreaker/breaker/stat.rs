use crate::{
    stat::{BucketWrap, LeapArray, MetricTrait},
    Result,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Debug, Default)]
pub struct Counter {
    pub(crate) target: AtomicU64,
    pub(crate) total: AtomicU64,
}

impl MetricTrait for Counter {
    fn reset(&self) {
        self.target.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
    }
}

pub type CounterLeapArray = LeapArray<Counter>;

impl CounterLeapArray {
    pub fn current_counter(&self) -> Result<Arc<BucketWrap<Counter>>> {
        // todo: redesign the structure, so that the wrapped value can be returned..
        // currently, it cannot be visited safely under an Arc
        self.current_bucket()
    }

    pub fn all_counter(&self) -> Vec<Arc<BucketWrap<Counter>>> {
        // todo: redesign the structure, so that the wrapped value can be returned..
        // currently, it cannot be visited safely under an Arc
        self.get_current_values()
    }
}

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicU32;

    use super::*;

    #[test]
    fn reset_bucket() {
        let counter = Counter {
            target: AtomicU64::new(5),
            total: AtomicU64::new(10),
        };
        counter.reset();
        assert_eq!(counter.target.load(Ordering::SeqCst), 0);
        assert_eq!(counter.total.load(Ordering::SeqCst), 0);
    }
}
