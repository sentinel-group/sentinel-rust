//! Directly reject

use super::*;
use crate::{
    base::{BlockType, ParamKey, TokenResult},
    utils,
};
use std::sync::{atomic::Ordering, Arc, Weak};

#[derive(Debug)]
pub struct RejectChecker<C: CounterTrait = Counter> {
    owner: Weak<Controller<C>>,
}

impl<C: CounterTrait> RejectChecker<C> {
    pub fn new() -> Self {
        RejectChecker { owner: Weak::new() }
    }
}

impl<C: CounterTrait> Default for RejectChecker<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: CounterTrait> Checker<C> for RejectChecker<C> {
    fn get_owner(&self) -> &Weak<Controller<C>> {
        &self.owner
    }

    fn set_owner(&mut self, owner: Weak<Controller<C>>) {
        self.owner = owner;
    }

    fn do_check(&self, arg: ParamKey, batch_count: u32) -> TokenResult {
        let owner = self.owner.upgrade().unwrap();
        let time_counter = &owner.metric.rule_time_counter;
        let token_counter = &owner.metric.rule_token_counter;
        if time_counter.cap() == 0 || token_counter.cap() == 0 {
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

        let max_count = token_count + owner.rule().burst_count;
        if batch_count as u64 > max_count {
            let msg = format!("hotspot reject check blocked, request batch count is more than max token count, arg: {:?}", arg);
            return TokenResult::new_blocked_with_cause(
                BlockType::HotSpotParamFlow,
                msg,
                owner.rule.clone(),
                Arc::new(batch_count),
            );
        }

        loop {
            let current_time_in_ms = utils::curr_time_millis();
            let last_add_token_time_arc =
                time_counter.add_if_absent(arg.clone(), current_time_in_ms);
            if last_add_token_time_arc.is_none() {
                // First fill token, and consume token immediately
                let left_count = max_count - batch_count as u64;
                token_counter.add_if_absent(arg, left_count);
                return TokenResult::new_pass();
            }
            let last_add_token_time_arc = last_add_token_time_arc.unwrap();

            // Calculate the time duration since last token was added.
            let pass_time =
                current_time_in_ms as i64 - last_add_token_time_arc.load(Ordering::SeqCst) as i64;
            if pass_time > (owner.rule().duration_in_sec * 1000) as i64 {
                // Refill the tokens because statistic window has passed.
                let left_count = max_count - batch_count as u64;
                let old_qps_arc = token_counter.add_if_absent(arg.clone(), left_count);
                if old_qps_arc.is_none() {
                    // Might not be accurate here.
                    last_add_token_time_arc.store(current_time_in_ms, Ordering::SeqCst);
                    return TokenResult::new_pass();
                }
                let old_qps_arc = old_qps_arc.unwrap();
                // refill token
                let rest_qps = old_qps_arc.load(Ordering::SeqCst);
                let to_add_token_num =
                    pass_time as u64 * token_count / (owner.rule().duration_in_sec * 1000);
                let new_qps = {
                    if to_add_token_num + rest_qps > max_count {
                        max_count as i64 - batch_count as i64
                    } else {
                        to_add_token_num as i64 + rest_qps as i64 - batch_count as i64
                    }
                };

                if new_qps < 0 {
                    let msg = format!("hotspot reject check blocked, request batch count is more than available token count, arg: {:?}", arg);
                    return TokenResult::new_blocked_with_cause(
                        BlockType::HotSpotParamFlow,
                        msg,
                        owner.rule.clone(),
                        Arc::new(token_count),
                    );
                }
                if old_qps_arc
                    .compare_exchange(
                        rest_qps,
                        new_qps as u64,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    last_add_token_time_arc.store(current_time_in_ms, Ordering::SeqCst);
                    return TokenResult::new_pass();
                }
                std::thread::yield_now();
            } else {
                //check whether the rest of token is enough to batch
                if let Some(old_qps_arc) = token_counter.get(&arg) {
                    let old_rest_token = old_qps_arc.load(Ordering::SeqCst);
                    if old_rest_token >= batch_count as u64 {
                        //update
                        if old_qps_arc
                            .compare_exchange(
                                old_rest_token,
                                old_rest_token - batch_count as u64,
                                Ordering::SeqCst,
                                Ordering::Relaxed,
                            )
                            .is_ok()
                        {
                            return TokenResult::new_pass();
                        }
                    } else {
                        let msg = format!("hotspot reject check blocked, request batch count is more than available token count, arg: {:?}", arg);
                        return TokenResult::new_blocked_with_cause(
                            BlockType::HotSpotParamFlow,
                            msg,
                            owner.rule.clone(),
                            Arc::new(token_count),
                        );
                    }
                }
                std::thread::yield_now();
            }
        }
    }
}
