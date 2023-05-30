//! Throttling indicates that pending requests will be throttled,
//! wait in queue (until free capacity is available)

use super::*;
use crate::{
    base::{BlockType, ParamKey, TokenResult},
    utils,
};
use std::sync::{atomic::Ordering, Arc, Weak};

#[derive(Debug)]
pub struct ThrottlingChecker<C: CounterTrait = Counter> {
    owner: Weak<Controller<C>>,
}

impl<C: CounterTrait> Default for ThrottlingChecker<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: CounterTrait> ThrottlingChecker<C> {
    pub fn new() -> Self {
        ThrottlingChecker { owner: Weak::new() }
    }
}

impl<C: CounterTrait> Checker<C> for ThrottlingChecker<C> {
    fn get_owner(&self) -> &Weak<Controller<C>> {
        &self.owner
    }

    fn set_owner(&mut self, owner: Weak<Controller<C>>) {
        self.owner = owner;
    }

    fn do_check(&self, arg: ParamKey, batch_count: u32) -> TokenResult {
        let owner = self.owner.upgrade().unwrap();
        let time_counter = &owner.metric.rule_time_counter;
        if time_counter.cap() == 0 {
            return TokenResult::new_pass();
        }
        let mut token_count = owner.rule.threshold;
        if let Some(val) = owner.rule.specific_items.get(&arg) {
            token_count = *val;
        }
        if token_count == 0 {
            let msg = format!("hotspot QPS check blocked, threshold is 0, arg: {:?}", arg);
            return TokenResult::new_blocked_with_cause(
                BlockType::HotSpotParamFlow,
                msg,
                owner.rule.clone(),
                Arc::new(token_count),
            );
        }

        let interval_cost_time = ((batch_count as u64 * owner.rule().duration_in_sec * 1000) as f64
            / token_count as f64)
            .round() as u64;
        loop {
            let current_time_in_ms = utils::curr_time_millis();
            let last_pass_time_arc = time_counter.add_if_absent(arg.clone(), current_time_in_ms);
            if last_pass_time_arc.is_none() {
                return TokenResult::new_pass();
            }
            let last_pass_time_arc = last_pass_time_arc.unwrap();
            let last_pass_time = last_pass_time_arc.load(Ordering::SeqCst);
            // calculate the expected pass time
            let expected_time = last_pass_time + interval_cost_time;

            if expected_time <= current_time_in_ms
                || expected_time - current_time_in_ms < owner.rule().max_queueing_time_ms
            {
                if last_pass_time_arc
                    .compare_exchange(
                        last_pass_time,
                        current_time_in_ms,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    let await_time = expected_time as i64 - current_time_in_ms as i64;
                    if await_time > 0 {
                        last_pass_time_arc.store(expected_time, Ordering::SeqCst);
                        return TokenResult::new_should_wait(await_time as u64);
                    } else {
                        return TokenResult::new_pass();
                    }
                } else {
                    std::thread::yield_now();
                }
            } else {
                let msg = format!("hotspot throttling check blocked, wait time exceedes max queueing time, arg: {:?}", arg);
                return TokenResult::new_blocked_with_cause(
                    BlockType::HotSpotParamFlow,
                    msg,
                    owner.rule.clone(),
                    Arc::new(token_count),
                );
            }
        }
    }
}
