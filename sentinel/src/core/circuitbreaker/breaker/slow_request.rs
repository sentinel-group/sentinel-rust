use super::*;
use crate::{base::EntryContext, logging, Result};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Debug)]
pub struct SlowRtBreaker {
    breaker: BreakerBase,
    max_allowed_rt: u64,
    max_slow_request_ratio: f64,
    min_request_amount: u64,
    stat: Arc<CounterLeapArray>,
}

impl SlowRtBreaker {
    pub fn new(rule: Arc<Rule>) -> Self {
        let interval = rule.stat_interval_ms;
        let bucket_count = rule.get_rule_stat_sliding_window_bucket_count();
        let stat = CounterLeapArray::new(bucket_count, interval).unwrap();
        Self::new_with_stat(rule, Arc::new(stat))
    }

    pub fn new_with_stat(rule: Arc<Rule>, stat: Arc<CounterLeapArray>) -> Self {
        let retry_timeout_ms = rule.retry_timeout_ms;
        let max_allowed_rt = rule.max_allowed_rt_ms;
        let max_slow_request_ratio = rule.threshold;
        let min_request_amount = rule.min_request_amount;
        Self {
            breaker: BreakerBase {
                rule,
                retry_timeout_ms,
                next_retry_timestamp_ms: AtomicU64::new(0),
                state: Arc::new(Mutex::new(State::default())),
            },
            max_allowed_rt,
            max_slow_request_ratio,
            min_request_amount,
            stat,
        }
    }
}

impl CircuitBreakerTrait for SlowRtBreaker {
    fn current_state(&self) -> State {
        self.breaker.current_state()
    }

    fn stat(&self) -> &Arc<CounterLeapArray> {
        &self.stat
    }

    fn bound_rule(&self) -> &Arc<Rule> {
        self.breaker.bound_rule()
    }

    fn try_pass(&self, ctx: Rc<RefCell<EntryContext>>) -> bool {
        match self.current_state() {
            State::Closed => true,
            State::Open => {
                self.breaker.retry_timeout_arrived() && self.breaker.from_open_to_half_open(ctx)
            }
            State::HalfOpen => false,
        }
    }

    fn on_request_complete(&self, rt: u64, _err: &Option<Error>) {
        let counter = self.stat.current_counter();
        if counter.is_err() {
            logging::error!(
                "Fail to get current counter in SlowRtBreaker#on_request_complete(). rule: {:?}",
                self.breaker.rule
            );
            return;
        }
        let counter = counter.unwrap();

        if rt > self.max_allowed_rt {
            counter.value().target.fetch_add(1, Ordering::SeqCst);
        }
        counter.value().target.fetch_add(1, Ordering::SeqCst);

        let mut slow_count = 0;
        let mut total_count = 0;
        let counters = self.stat.all_counter();
        for c in counters {
            slow_count += c.value().target.load(Ordering::SeqCst);
            total_count += c.value().total.load(Ordering::SeqCst);
        }

        let slow_ratio = slow_count as f64 / total_count as f64;
        // handle state changes when threshold exceeded
        match self.current_state() {
            State::HalfOpen => {
                if rt > self.max_allowed_rt {
                    // fail to probe
                    self.breaker.from_half_open_to_open(Arc::new(1.0));
                } else {
                    // succeed to probe
                    self.breaker.from_half_open_to_closed();
                    self.reset_metric();
                }
            }
            State::Closed => {
                if total_count >= self.min_request_amount
                    && slow_ratio >= self.max_slow_request_ratio
                {
                    match self.current_state() {
                        State::Closed => {
                            self.breaker.from_closed_to_open(Arc::new(slow_ratio));
                        }
                        State::HalfOpen => {
                            self.breaker.from_half_open_to_open(Arc::new(slow_ratio));
                        }
                        State::Open => {}
                    }
                }
            }
            State::Open => {}
        }
    }

    fn reset_metric(&self) {
        for c in self.stat.all_counter() {
            c.value().reset()
        }
    }
}
