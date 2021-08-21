pub mod reject;
pub mod throttling;

pub use reject::*;
pub use throttling::*;

use super::*;
use crate::{
    base::{BlockType, EntryContext, ParamKey, TokenResult},
    logging, utils, Error, Result,
};
use lazy_static::lazy_static;
use std::any::Any;
use std::cell::RefCell;
use std::cmp::min;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{atomic::Ordering, Arc, Mutex, RwLock, Weak};

/// Traffic Shaping `Checker` performs checking according to current metrics and the traffic
/// shaping strategy, then yield the token result.
pub trait Checker: Send + Sync + std::fmt::Debug {
    fn get_owner(&self) -> &Weak<Controller>;
    fn set_owner(&mut self, owner: Weak<Controller>);
    fn do_check(&self, arg: ParamKey, batch_count: u32) -> TokenResult;
}

#[derive(Debug)]
pub struct Controller {
    rule: Arc<Rule>,
    metric: Arc<ParamsMetric>,
    checker: Option<Arc<Mutex<dyn Checker>>>,
}

impl Controller {
    pub fn new(rule: Arc<Rule>) -> Controller {
        let metric = match rule.metric_type {
            MetricType::QPS => {
                let mut capacity = {
                    if rule.params_max_capacity > 0 {
                        rule.params_max_capacity
                    } else if rule.duration_in_sec == 0 {
                        PARAMS_MAX_CAPACITY
                    } else {
                        min(
                            PARAMS_MAX_CAPACITY,
                            PARAMS_CAPACITY_BASE * rule.duration_in_sec as usize,
                        )
                    }
                };
                ParamsMetric {
                    rule_time_counter: Counter::new(capacity),
                    rule_token_counter: Counter::new(capacity),
                    ..Default::default()
                }
            }
            MetricType::Concurrency => {
                let mut capacity = {
                    if rule.params_max_capacity > 0 {
                        rule.params_max_capacity
                    } else {
                        CONCURRENCY_MAX_COUNT
                    }
                };
                ParamsMetric {
                    concurrency_counter: Counter::new(capacity),
                    ..Default::default()
                }
            }
        };
        Self::new_with_metric(rule, Arc::new(metric))
    }

    pub fn new_with_metric(rule: Arc<Rule>, metric: Arc<ParamsMetric>) -> Controller {
        Controller {
            rule,
            metric,
            checker: None,
        }
    }

    pub fn get_checker(&self) -> &Arc<Mutex<dyn Checker>> {
        self.checker.as_ref().unwrap()
    }

    pub fn set_checker(&mut self, checker: Arc<Mutex<dyn Checker>>) {
        self.checker = Some(checker);
    }

    pub fn metric(&self) -> &Arc<ParamsMetric> {
        &self.metric
    }

    pub fn rule(&self) -> &Arc<Rule> {
        &self.rule
    }

    pub fn param_index(&self) -> isize {
        self.rule.param_index
    }

    pub fn perform_checking(&self, arg: ParamKey, batch_count: u32) -> TokenResult {
        match self.rule.metric_type {
            MetricType::Concurrency => self.perform_checking_for_concurrency_metric(arg),
            MetricType::QPS => {
                let checker = self.checker.as_ref().unwrap();
                let checker = checker.lock().unwrap();
                checker.do_check(arg, batch_count)
            }
        }
    }

    pub fn perform_checking_for_concurrency_metric(&self, arg: ParamKey) -> TokenResult {
        let last_concurrency = self
            .metric
            .concurrency_counter
            .add_if_absent(arg.clone(), 0);
        if last_concurrency.is_none() {
            return TokenResult::new_pass();
        }
        let concurrency = last_concurrency.unwrap().load(Ordering::SeqCst) + 1;

        let threshold = {
            if let Some(specific_concurrency) = self.rule.specific_items.get(&arg) {
                *specific_concurrency
            } else {
                self.rule.threshold
            }
        };
        if concurrency <= threshold {
            return TokenResult::new_pass();
        } else {
            let msg = format!("hotspot specific concurrency check blocked, arg: {:?}", arg);
            return TokenResult::new_blocked_with_cause(
                BlockType::HotSpotParamFlow,
                msg,
                self.rule.clone(),
                Arc::new(concurrency),
            );
        }
    }

    /// ExtractArgs matches the arg from ctx based on Controller
    pub fn extract_args(&self, ctx: &Rc<RefCell<EntryContext>>) -> Option<ParamKey> {
        if let Some(args) = self.extract_kv_args(ctx) {
            Some(args)
        } else if let Some(args) = self.extract_list_args(ctx) {
            Some(args)
        } else {
            None
        }
    }

    fn extract_list_args(&self, ctx: &Rc<RefCell<EntryContext>>) -> Option<ParamKey> {
        let ctx = ctx.borrow();
        let args = ctx.input().args();
        match args {
            Some(args) => {
                let mut idx = self.rule.param_index;
                if idx < 0 {
                    idx = args.len() as isize + idx;
                }
                if idx < 0 {
                    logging::debug!("[extract_args] The param index of hotspot traffic shaping controller is invalid, args: {:?}, param_index: {}", args, self.param_index());
                    None
                } else if idx as usize >= args.len() {
                    logging::debug!("[extract_args] The argument in index doesn't exist, args: {:?}, param_index: {}", args, self.param_index());
                    None
                } else {
                    Some(args[idx as usize].clone())
                }
            }
            None => {
                logging::debug!("[extract_args] The args of ctx is None");
                None
            }
        }
    }

    fn extract_kv_args(&self, ctx: &Rc<RefCell<EntryContext>>) -> Option<ParamKey> {
        let ctx = ctx.borrow();
        let attachments = ctx.input().attachments();
        match attachments {
            Some(attachments) => {
                let key = self.rule.param_key.trim();
                if key.len() == 0 {
                    logging::debug!(
                        "[extract_args] The param key is invalid, key: {}",
                        self.rule.param_key
                    );
                    None
                } else if !attachments.contains_key(key) {
                    logging::debug!("[extract_args] The extracted data does not exist, key: {:?}, attachments: {:?}", self.rule.param_key, attachments);
                    None
                } else {
                    Some(attachments[key].clone())
                }
            }
            None => {
                logging::debug!("[extract_args] The attachments of ctx is None");
                None
            }
        }
    }
}
