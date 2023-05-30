//! Throttling indicates that pending requests will be throttled,
//! wait in queue (until free capacity is available)

use super::{Checker, Controller, Rule};
use crate::base::{BlockType, StatNode, TokenResult};
use crate::utils;
use std::convert::TryInto;
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc, Weak,
};

static BLOCK_MSG_QUEUEING: &str = "flow throttling check blocked, threshold is <= 0.0";

#[derive(Debug)]
pub struct ThrottlingChecker {
    owner: Weak<Controller>,
    max_queueing_time_ns: i64,
    stat_interval_ns: i64,
    last_passed_time: AtomicI64,
}

impl ThrottlingChecker {
    pub fn new(owner: Weak<Controller>, rule: Arc<Rule>) -> Self {
        let timeout_ms = rule.max_queueing_time_ms;
        let stat_interval_ms = rule.stat_interval_ms;

        let stat_interval_ns = {
            if stat_interval_ms == 0 {
                utils::milli2nano(1000)
            } else {
                utils::milli2nano(stat_interval_ms)
            }
        }
        .try_into()
        .unwrap();
        ThrottlingChecker {
            owner,
            max_queueing_time_ns: utils::milli2nano(timeout_ms).try_into().unwrap(),
            stat_interval_ns,
            last_passed_time: AtomicI64::new(0),
        }
    }
}

impl Checker for ThrottlingChecker {
    fn get_owner(&self) -> &Weak<Controller> {
        &self.owner
    }

    fn set_owner(&mut self, owner: Weak<Controller>) {
        self.owner = owner;
    }

    fn do_check(
        &self,
        _stat_node: Option<Arc<dyn StatNode>>,
        batch_count: u32,
        threshold: f64,
    ) -> TokenResult {
        // Pass when batch count is less or equal than 0.
        if batch_count == 0 {
            return TokenResult::new_pass();
        }
        let owner = self.owner.upgrade();

        if threshold <= 0.0 {
            match owner {
                Some(owner) => {
                    return TokenResult::new_blocked_with_cause(
                        BlockType::Flow,
                        BLOCK_MSG_QUEUEING.into(),
                        owner.rule().clone(),
                        Arc::new(threshold),
                    );
                }
                None => {
                    return TokenResult::new_blocked_with_msg(
                        BlockType::Flow,
                        BLOCK_MSG_QUEUEING.into(),
                    );
                }
            }
        }
        let batch_count = batch_count as f64;
        if batch_count > threshold {
            return TokenResult::new_blocked(BlockType::Flow);
        }

        // Here we use nanosecond so that we could control the queueing time more accurately.
        let curr_nano: i64 = utils::curr_time_nanos().try_into().unwrap();

        // The interval between two requests (in nanoseconds).
        let interval_ns = (batch_count.ceil() / threshold * (self.stat_interval_ns as f64)) as i64;

        let loaded_last_passed_time = self.last_passed_time.load(Ordering::SeqCst);
        // Expected pass time of this request.
        let expected_time = loaded_last_passed_time + interval_ns;
        // It has been more than `interval_ns` not running this task
        if expected_time <= curr_nano
            && self
                .last_passed_time
                .compare_exchange(
                    loaded_last_passed_time,
                    curr_nano,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                )
                .is_ok()
        {
            return TokenResult::new_pass();
        }
        // It has been run recently, need queueing, check queueing time
        let estimated_queue_duration =
            self.last_passed_time.load(Ordering::SeqCst) + interval_ns - curr_nano;
        if estimated_queue_duration > self.max_queueing_time_ns {
            match owner {
                Some(owner) => {
                    return TokenResult::new_blocked_with_cause(
                        BlockType::Flow,
                        BLOCK_MSG_QUEUEING.into(),
                        owner.rule().clone(),
                        Arc::new(estimated_queue_duration),
                    );
                }
                None => {
                    return TokenResult::new_blocked_with_msg(
                        BlockType::Flow,
                        BLOCK_MSG_QUEUEING.into(),
                    );
                }
            }
        }
        // It is expected to run at `expected_time`
        let expected_time = self
            .last_passed_time
            .fetch_add(interval_ns, Ordering::SeqCst)
            + interval_ns;
        let estimated_queue_duration = expected_time - curr_nano;
        if estimated_queue_duration > self.max_queueing_time_ns {
            // Subtract the interval.
            self.last_passed_time
                .fetch_sub(interval_ns, Ordering::SeqCst);
            match owner {
                Some(owner) => {
                    return TokenResult::new_blocked_with_cause(
                        BlockType::Flow,
                        BLOCK_MSG_QUEUEING.into(),
                        owner.rule().clone(),
                        Arc::new(estimated_queue_duration),
                    );
                }
                None => {
                    return TokenResult::new_blocked_with_msg(
                        BlockType::Flow,
                        BLOCK_MSG_QUEUEING.into(),
                    );
                }
            }
        }
        if estimated_queue_duration > 0 {
            TokenResult::new_should_wait(estimated_queue_duration.try_into().unwrap())
        } else {
            TokenResult::new_should_wait(0)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::unix_time_unit_offset;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn single_thread_no_queueing() {
        let interval_ms = 10000;
        let threshold = 50.0;
        let timeout_ms = 0;
        let rule = Arc::new(Rule {
            max_queueing_time_ms: timeout_ms,
            stat_interval_ms: interval_ms,
            ..Default::default()
        });

        let tc = ThrottlingChecker::new(Weak::new(), rule);

        // Should block when batchCount > threshold.
        let res = tc.do_check(None, (threshold + 1.0) as u32, threshold);
        assert!(res.is_blocked());

        // The first request will pass.
        let res = tc.do_check(None, threshold as u32, threshold);
        assert!(res.is_pass());

        let req_count = 10;
        for _ in 0..req_count {
            assert!(tc.do_check(None, 1, threshold).is_blocked());
        }
        utils::sleep_for_ms(interval_ms as u64 / threshold as u64 * req_count + 10);

        assert!(tc.do_check(None, 1, threshold).is_pass());
        assert!(tc.do_check(None, 1, threshold).is_blocked());
    }

    #[test]
    fn single_thread() {
        let interval_ms = 10000;
        let threshold = 50.0;
        let timeout_ms = 2000;
        let rule = Arc::new(Rule {
            max_queueing_time_ms: timeout_ms,
            stat_interval_ms: interval_ms,
            ..Default::default()
        });

        let tc = ThrottlingChecker::new(Weak::new(), rule);

        // Should block when batchCount > threshold.
        let res = tc.do_check(None, (threshold + 1.0) as u32, threshold);
        assert!(res.is_blocked());

        // The first request will pass.
        let res = tc.do_check(None, threshold as u32, threshold);
        assert!(res.is_pass());

        let req_count: usize = 20;
        let mut result_list = Vec::<TokenResult>::with_capacity(req_count);
        for _ in 0..req_count {
            let res = tc.do_check(None, 1, threshold);
            result_list.push(res);
        }

        // todo: estimated queueing time is not accurate currently
        const EPSILON: f64 = 2.0;
        // wait_count is count of request that will wait and not be blocked
        let wait_count: u64 = timeout_ms as u64 / (interval_ms as f64 / threshold) as u64;
        for (i, result) in result_list.iter().enumerate().take(wait_count as usize) {
            assert!(result.is_wait());
            let wt = result.nanos_to_wait() as f64;
            let mid = ((i + 1) as u64 * 1000 * unix_time_unit_offset() / wait_count) as f64;
            assert!(wt > (1.0 - EPSILON) * mid && wt < (1.0 + EPSILON) * mid);
        }
        for result in result_list.iter().take(req_count).skip(wait_count as usize) {
            assert!(result.is_blocked());
        }
    }

    #[test]
    fn parallel_queueing() {
        let interval_ms = 10000;
        let threshold = 50.0;
        let timeout_ms = 2000;
        let rule = Arc::new(Rule {
            max_queueing_time_ms: timeout_ms,
            stat_interval_ms: interval_ms,
            ..Default::default()
        });

        let tc = Arc::new(ThrottlingChecker::new(Weak::new(), rule));

        assert!(tc.do_check(None, 1, threshold).is_pass());
        let thread_num: u32 = 24;
        let mut handles = Vec::with_capacity(thread_num as usize);
        let wait_count = Arc::new(AtomicU32::new(0));
        let block_count = Arc::new(AtomicU32::new(0));
        for _ in 0..thread_num {
            let tc_clone = Arc::clone(&tc);
            let block_clone = Arc::clone(&block_count);
            let wait_clone = Arc::clone(&wait_count);
            handles.push(std::thread::spawn(move || {
                let res = tc_clone.do_check(None, 1, threshold);
                if res.is_blocked() {
                    block_clone.fetch_add(1, Ordering::SeqCst);
                } else if res.is_wait() {
                    wait_clone.fetch_add(1, Ordering::SeqCst);
                } else {
                    panic!("Should not pass.");
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(
            thread_num,
            wait_count.load(Ordering::SeqCst) + block_count.load(Ordering::SeqCst)
        );
        const DELTA: u32 = 1;
        assert!(
            10 - DELTA <= wait_count.load(Ordering::SeqCst)
                && wait_count.load(Ordering::SeqCst) <= 10 + DELTA
        );
    }

    #[test]
    #[ignore]
    // todo: this test should not be ignored for single-thread,
    // but currently, it fails when run with others tests.
    // If run this test alone, it won't panic...
    fn parallel_pass() {
        let interval_ms = 10000;
        let threshold = 50.0;
        let timeout_ms = 0;
        let rule = Arc::new(Rule {
            max_queueing_time_ms: timeout_ms,
            stat_interval_ms: interval_ms,
            ..Default::default()
        });

        let tc = Arc::new(ThrottlingChecker::new(Weak::new(), rule));

        let thread_num: u32 = 512;
        let mut handles = Vec::with_capacity(thread_num as usize);
        let pass_count = Arc::new(AtomicU32::new(0));

        for _ in 0..thread_num {
            let tc_clone = Arc::clone(&tc);
            let pass_clone = Arc::clone(&pass_count);
            handles.push(std::thread::spawn(move || {
                let res = tc_clone.do_check(None, 1, threshold);
                if res.is_pass() {
                    pass_clone.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(1, pass_count.load(Ordering::SeqCst));
    }
}
