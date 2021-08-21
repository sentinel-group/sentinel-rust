use super::Rule;
///! `WarmUpCalculator` is based on the **Token Bucket** algorithm
use super::{Calculator, Checker, Controller};
use crate::base::{BlockType, MetricEvent, TokenResult};
use crate::{config, logging, utils};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, Weak,
};

#[derive(Debug)]
pub struct WarmUpCalculator {
    owner: Weak<Controller>,
    threshold: f64,
    cold_factor: u32,
    warning_token: u64,
    max_token: u64,
    slope: f64,
    stored_tokens: AtomicU64,
    last_filled_time: AtomicU64,
}

impl WarmUpCalculator {
    pub fn new(owner: Weak<Controller>, rule: Arc<Rule>) -> Self {
        let mut cold_factor = rule.warm_up_cold_factor;
        if cold_factor <= 1 {
            logging::warn!("[WarmUpCalculator::new] Not set warm_up_cold_factor,use default warm up cold factor value: {}", config::WARM_UP_COLD_FACTOR);
            cold_factor = config::WARM_UP_COLD_FACTOR;
        };
        let threshold = rule.threshold;
        let warm_up_period = rule.warm_up_period_sec as f64;

        let cold_factor_plus = (cold_factor + 1) as f64;
        let cold_factor_minus = (cold_factor - 1) as f64;
        let warning_token = (warm_up_period * threshold / cold_factor_minus) as u64;
        let max_token = warning_token + 2 * (warm_up_period * threshold / cold_factor_plus) as u64;
        let slope = cold_factor_minus / threshold / (max_token - warning_token) as f64;

        WarmUpCalculator {
            owner,
            cold_factor,
            warning_token,
            max_token,
            slope,
            threshold,
            stored_tokens: AtomicU64::new(0),
            last_filled_time: AtomicU64::new(0),
        }
    }

    fn sync_token(&self, pass_qps: f64) {
        let mut curr_time = utils::curr_time_millis();
        curr_time = curr_time - curr_time % 1000;

        let old_last_fill_time = self.last_filled_time.load(Ordering::SeqCst);
        if curr_time <= old_last_fill_time {
            return;
        }

        let old_value = self.stored_tokens.load(Ordering::SeqCst);
        let new_value = self.cool_down_tokens(curr_time, pass_qps);

        if self
            .stored_tokens
            .compare_exchange(old_value, new_value, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            let prev_value = self
                .stored_tokens
                .fetch_sub(pass_qps as u64, Ordering::SeqCst);
            if prev_value < pass_qps as u64 {
                // `prev_value < pass_qps` means that overflow has happened
                self.stored_tokens.store(0, Ordering::SeqCst);
            }
            self.last_filled_time.store(curr_time, Ordering::SeqCst);
        }
    }

    fn cool_down_tokens(&self, curr_time: u64, pass_qps: f64) -> u64 {
        let old_value = self.stored_tokens.load(Ordering::SeqCst);
        let mut new_value = old_value;
        let last_time = self.last_filled_time.load(Ordering::SeqCst);
        // Prerequisites for adding a token:
        // When token consumption is much lower than the warning line
        if old_value < self.warning_token
            || pass_qps < (self.threshold / self.cold_factor as f64).floor()
        {
            new_value =
                old_value + ((curr_time - last_time) as f64 * self.threshold / 1000.0) as u64;
        }

        std::cmp::min(new_value, self.max_token)
    }
}

impl Calculator for WarmUpCalculator {
    fn get_owner(&self) -> &Weak<Controller> {
        &self.owner
    }

    fn set_owner(&mut self, owner: Weak<Controller>) {
        self.owner = owner;
    }

    fn calculate_allowed_threshold(&self, _batch_count: u32, _flag: i32) -> f64 {
        let owner = self.owner.upgrade().unwrap();
        let read_only_metric = owner.stat().read_only_metric();
        let previous_qps = read_only_metric.qps_previous(MetricEvent::Pass);
        self.sync_token(previous_qps);
        let rest_token = self.stored_tokens.load(Ordering::SeqCst);

        if rest_token >= self.warning_token {
            let above_token = rest_token - self.warning_token;
            // compute warning QPS
            utils::next_after(1.0 / (above_token as f64 * self.slope + 1.0 / self.threshold))
        } else {
            self.threshold
        }
    }
}
