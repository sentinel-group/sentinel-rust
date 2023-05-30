use super::*;
use crate::logging;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Debug)]
pub struct ErrorCountBreaker {
    breaker: BreakerBase,
    min_request_amount: u64,
    error_count_threshold: u64,
    // stat needs to be shared, so we take Arc
    stat: Arc<CounterLeapArray>,
}

impl ErrorCountBreaker {
    pub fn new(rule: Arc<Rule>) -> Self {
        let interval = rule.stat_interval_ms;
        let bucket_count = rule.get_rule_stat_sliding_window_bucket_count();
        let stat = CounterLeapArray::new(bucket_count, interval).unwrap();
        Self::new_with_stat(rule, Arc::new(stat))
    }

    pub fn new_with_stat(rule: Arc<Rule>, stat: Arc<CounterLeapArray>) -> Self {
        let retry_timeout_ms = rule.retry_timeout_ms;
        let min_request_amount = rule.min_request_amount;
        let error_count_threshold = rule.threshold as u64;
        Self {
            breaker: BreakerBase {
                rule,
                retry_timeout_ms,
                next_retry_timestamp_ms: AtomicU64::new(0),
                state: Arc::new(Mutex::new(State::default())),
            },
            min_request_amount,
            error_count_threshold,
            stat,
        }
    }
}

impl CircuitBreakerTrait for ErrorCountBreaker {
    fn breaker(&self) -> &BreakerBase {
        &self.breaker
    }

    fn stat(&self) -> &Arc<CounterLeapArray> {
        &self.stat
    }

    fn on_request_complete(&self, _rt: u64, err: &Option<Error>) {
        let counter = self.stat.current_counter();
        if counter.is_err() {
            logging::error!("Fail to get current counter in ErrorCountBreaker#on_request_complete(). rule: {:?}", self.breaker.rule);
            return;
        }
        let counter = counter.unwrap();

        if err.is_some() {
            counter.value().target.fetch_add(1, Ordering::SeqCst);
        }
        counter.value().total.fetch_add(1, Ordering::SeqCst);

        let mut error_count = 0;
        let mut total_count = 0;
        let counters = self.stat.all_counter();
        for c in counters {
            error_count += c.value().target.load(Ordering::SeqCst);
            total_count += c.value().total.load(Ordering::SeqCst);
        }

        // handle state changes when threshold exceeded
        match self.current_state() {
            State::HalfOpen => {
                if err.is_none() {
                    self.breaker.from_half_open_to_closed();
                    self.reset_metric();
                } else {
                    self.breaker.from_half_open_to_open(Arc::new(1));
                }
            }
            State::Closed => {
                if total_count >= self.min_request_amount
                    && error_count >= self.error_count_threshold
                {
                    match self.current_state() {
                        State::Closed => {
                            self.breaker.from_closed_to_open(Arc::new(error_count));
                        }
                        State::HalfOpen => {
                            self.breaker.from_half_open_to_open(Arc::new(error_count));
                        }
                        State::Open => {}
                    }
                }
            }
            State::Open => {}
        }
    }
}
