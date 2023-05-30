pub mod reject;
pub mod throttling;

pub use reject::*;
pub use throttling::*;

use super::*;
use crate::{
    base::{BlockType, EntryContext, ParamKey, TokenResult},
    logging,
};
use std::cmp::min;
use std::sync::{atomic::Ordering, Arc, Mutex, Weak};

/// Traffic Shaping `Checker` performs checking according to current metrics and the traffic
/// shaping strategy, then yield the token result.
pub trait Checker<C: CounterTrait = Counter>: Send + Sync + std::fmt::Debug {
    fn get_owner(&self) -> &Weak<Controller<C>>;
    fn set_owner(&mut self, owner: Weak<Controller<C>>);
    fn do_check(&self, arg: ParamKey, batch_count: u32) -> TokenResult;
}

// The generic is
#[derive(Debug)]
pub struct Controller<C = Counter<ParamKey>>
where
    C: CounterTrait,
{
    rule: Arc<Rule>,
    metric: Arc<ParamsMetric<C>>,
    checker: Option<Arc<Mutex<dyn Checker<C>>>>,
}

impl<C> Controller<C>
where
    C: CounterTrait,
{
    /// Please refer to the generators in the `rule_manager`
    pub fn new(rule: Arc<Rule>) -> Controller<C> {
        let metric = match rule.metric_type {
            MetricType::QPS => {
                let capacity = {
                    if rule.params_max_capacity > 0 {
                        rule.params_max_capacity
                    } else if rule.duration_in_sec == 0 {
                        // in fact, this invalid rule won't be loaded
                        PARAMS_MAX_CAPACITY
                    } else {
                        min(
                            PARAMS_MAX_CAPACITY,
                            PARAMS_CAPACITY_BASE * rule.duration_in_sec as usize,
                        )
                    }
                };
                ParamsMetric {
                    rule_time_counter: C::with_capacity(capacity),
                    rule_token_counter: C::with_capacity(capacity),
                    ..Default::default()
                }
            }
            MetricType::Concurrency => {
                let capacity = {
                    if rule.params_max_capacity > 0 {
                        rule.params_max_capacity
                    } else {
                        CONCURRENCY_MAX_COUNT
                    }
                };
                ParamsMetric {
                    concurrency_counter: C::with_capacity(capacity),
                    ..Default::default()
                }
            }
        };
        Self::new_with_metric(rule, Arc::new(metric))
    }

    /// Please refer to the generators in the `rule_manager`
    pub fn new_with_metric(rule: Arc<Rule>, metric: Arc<ParamsMetric<C>>) -> Controller<C> {
        Controller {
            rule,
            metric,
            checker: None,
        }
    }

    pub fn get_checker(&self) -> &Arc<Mutex<dyn Checker<C>>> {
        self.checker.as_ref().unwrap()
    }

    pub fn set_checker(&mut self, checker: Arc<Mutex<dyn Checker<C>>>) {
        self.checker = Some(checker);
    }

    pub fn metric(&self) -> &Arc<ParamsMetric<C>> {
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
            // settings stored in the `specific_items` is prior to the `threshold` in `rule`
            if let Some(specific_concurrency) = self.rule.specific_items.get(&arg) {
                *specific_concurrency
            } else {
                self.rule.threshold
            }
        };
        if concurrency <= threshold {
            TokenResult::new_pass()
        } else {
            let msg = format!("hotspot specific concurrency check blocked, arg: {:?}", arg);
            TokenResult::new_blocked_with_cause(
                BlockType::HotSpotParamFlow,
                msg,
                self.rule.clone(),
                Arc::new(concurrency),
            )
        }
    }

    /// ExtractArgs matches the arg from ctx based on Controller
    pub fn extract_args(&self, ctx: &EntryContext) -> Option<ParamKey> {
        if let Some(args) = self.extract_kv_args(ctx) {
            Some(args)
        } else {
            self.extract_list_args(ctx)
        }
    }

    fn extract_list_args(&self, ctx: &EntryContext) -> Option<ParamKey> {
        let args = ctx.input().args();
        match args {
            Some(args) => {
                let mut idx = self.rule.param_index;
                if idx < 0 {
                    idx += args.len() as isize;
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

    fn extract_kv_args(&self, ctx: &EntryContext) -> Option<ParamKey> {
        let attachments = ctx.input().attachments();
        match attachments {
            Some(attachments) => {
                let key = self.rule.param_key.trim();
                if key.is_empty() {
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

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::{
        base::{EntryContext, ParamsList, ParamsMap, SentinelInput},
        utils,
    };
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn construct_concurrency() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);
        for i in 0..CONCURRENCY_MAX_COUNT + 5 {
            controller
                .metric()
                .concurrency_counter
                .add_if_absent(i.to_string(), 0);
        }
        assert_eq!(
            CONCURRENCY_MAX_COUNT,
            controller.metric().concurrency_counter.len()
        );

        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            params_max_capacity: 100,
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);
        for i in 0..CONCURRENCY_MAX_COUNT + 5 {
            controller
                .metric()
                .concurrency_counter
                .add_if_absent(i.to_string(), 0);
        }
        assert_eq!(100, controller.metric().concurrency_counter.len());
    }

    #[test]
    fn construct_qps() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 10,
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);
        for i in 0..30000 {
            controller
                .metric()
                .rule_token_counter
                .add_if_absent(i.to_string(), 0);
            controller
                .metric()
                .rule_time_counter
                .add_if_absent(i.to_string(), 0);
        }
        assert_eq!(
            PARAMS_MAX_CAPACITY,
            controller.metric().rule_token_counter.len()
        );
        assert_eq!(
            PARAMS_MAX_CAPACITY,
            controller.metric().rule_time_counter.len()
        );
    }

    #[test]
    fn extract_args_none() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);

        let args = ParamsList::new();
        let attachments = ParamsMap::new();

        let mut ctx = EntryContext::new();
        let mut input = SentinelInput::new(1, 0);
        input.set_args(args);
        input.set_attachments(attachments);
        ctx.set_input(input);

        // no data
        let extracted = controller.extract_args(&ctx);
        assert!(extracted.is_none());
    }

    #[test]
    fn extract_args_exist() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            param_index: 0,
            param_key: "test1".into(),
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);

        let args = vec!["1".into(), "2".into()];
        let mut attachments = ParamsMap::new();
        attachments.insert("test1".into(), "v1".into());

        let mut ctx = EntryContext::new();
        let mut input = SentinelInput::new(1, 0);
        input.set_args(args);
        input.set_attachments(attachments);
        ctx.set_input(input);

        let extracted = controller.extract_args(&ctx);
        assert_eq!("v1", &extracted.unwrap());
    }

    #[test]
    fn extract_args_exist_kv() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            param_index: 10,
            param_key: "test1".into(),
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);

        let args = vec!["1".into(), "2".into()];
        let mut attachments = ParamsMap::new();
        attachments.insert("test1".into(), "v1".into());

        let mut ctx = EntryContext::new();
        let mut input = SentinelInput::new(1, 0);
        input.set_args(args);
        input.set_attachments(attachments);
        ctx.set_input(input);

        let extracted = controller.extract_args(&ctx);
        assert_eq!("v1", &extracted.unwrap());
    }

    #[test]
    fn extract_args_exist_list() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            param_index: 1,
            param_key: "test2".into(),
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);

        let args = vec!["1".into(), "2".into()];
        let mut attachments = ParamsMap::new();
        attachments.insert("test1".into(), "v1".into());

        let mut ctx = EntryContext::new();
        let mut input = SentinelInput::new(1, 0);
        input.set_args(args);
        input.set_attachments(attachments);
        ctx.set_input(input);

        let extracted = controller.extract_args(&ctx);
        assert_eq!("2", &extracted.unwrap());
    }

    #[test]
    fn extract_args_not_exist() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Reject,
            duration_in_sec: 1,
            param_index: 10,
            param_key: "test2".into(),
            ..Default::default()
        });
        let controller = gen_reject::<Counter>(rule, None);

        let args = vec!["1".into(), "2".into()];
        let mut attachments = ParamsMap::new();
        attachments.insert("test1".into(), "v1".into());

        let mut ctx = EntryContext::new();
        let mut input = SentinelInput::new(1, 0);
        input.set_args(args);
        input.set_attachments(attachments);
        ctx.set_input(input);

        let extracted = controller.extract_args(&ctx);
        assert!(extracted.is_none());
    }

    mod reject {
        use super::*;

        mod check_concurrency {
            use super::*;

            #[test]
            fn threshold() {
                let rule = Arc::new(Rule {
                    resource: "abc".into(),
                    metric_type: MetricType::Concurrency,
                    control_strategy: ControlStrategy::Reject,
                    threshold: 100,
                    duration_in_sec: 1,
                    ..Default::default()
                });

                let concurrency = Arc::new(AtomicU64::new(0));
                let mut concurrency_counter: MockCounter<ParamKey> = MockCounter::new();
                concurrency_counter
                    .expect_add_if_absent()
                    .return_const(Some(Arc::clone(&concurrency)));
                let metric = Arc::new(ParamsMetric {
                    concurrency_counter,
                    ..Default::default()
                });

                let controller = gen_reject(rule, Some(metric));
                let token = controller.perform_checking_for_concurrency_metric(666688.to_string());
                assert!(token.is_pass());

                concurrency.store(101, Ordering::SeqCst);
                let token = controller.perform_checking_for_concurrency_metric(666688.to_string());
                assert!(token.is_blocked());
            }

            #[test]
            fn args() {
                let mut specific_items = HashMap::new();
                specific_items.insert(666688.to_string(), 20);
                let rule = Arc::new(Rule {
                    resource: "abc".into(),
                    metric_type: MetricType::Concurrency,
                    control_strategy: ControlStrategy::Reject,
                    threshold: 100,
                    duration_in_sec: 1,
                    specific_items,
                    ..Default::default()
                });

                let concurrency = Arc::new(AtomicU64::new(50));
                let mut concurrency_counter: MockCounter<ParamKey> = MockCounter::new();
                concurrency_counter
                    .expect_add_if_absent()
                    .times(2)
                    .return_const(Some(Arc::clone(&concurrency)));
                let metric = Arc::new(ParamsMetric {
                    concurrency_counter,
                    ..Default::default()
                });

                let controller = gen_reject(rule, Some(metric));
                let token = controller.perform_checking_for_concurrency_metric(666688.to_string());
                assert!(token.is_blocked());

                concurrency.store(10, Ordering::SeqCst);
                let token = controller.perform_checking_for_concurrency_metric(666688.to_string());
                assert!(token.is_pass());
            }
        }

        mod check_qps {
            use super::*;

            #[test]
            fn time_counter_none() {
                let rule = Arc::new(Rule {
                    resource: "abc".into(),
                    metric_type: MetricType::QPS,
                    control_strategy: ControlStrategy::Reject,
                    threshold: 100,
                    duration_in_sec: 1,
                    burst_count: 10,
                    ..Default::default()
                });

                let mut rule_time_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_time_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(None);
                rule_time_counter
                    .expect_cap()
                    .times(2)
                    .return_const(PARAMS_MAX_CAPACITY);
                let mut rule_token_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_token_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(None);
                rule_token_counter
                    .expect_cap()
                    .times(2)
                    .return_const(PARAMS_MAX_CAPACITY);
                let metric = Arc::new(ParamsMetric {
                    rule_time_counter,
                    rule_token_counter,
                    ..Default::default()
                });

                let controller = gen_reject(rule, Some(metric));
                let token = controller.perform_checking(10110.to_string(), 130);
                assert!(token.is_blocked());

                let token = controller.perform_checking(10110.to_string(), 20);
                assert!(token.is_pass());
            }

            #[test]
            fn subtract_token() {
                let rule = Arc::new(Rule {
                    resource: "abc".into(),
                    metric_type: MetricType::QPS,
                    control_strategy: ControlStrategy::Reject,
                    threshold: 100,
                    duration_in_sec: 10,
                    burst_count: 10,
                    ..Default::default()
                });
                let old_qps = Arc::new(AtomicU64::new(50));
                let last_add_token_time =
                    Arc::new(AtomicU64::new(utils::curr_time_millis() - 1000));
                let mut rule_time_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_time_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(Some(Arc::clone(&last_add_token_time)));
                rule_time_counter
                    .expect_cap()
                    .once()
                    .return_const(PARAMS_MAX_CAPACITY);
                let mut rule_token_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_token_counter
                    .expect_get::<ParamKey>()
                    .once()
                    .return_const(Some(Arc::clone(&old_qps)));
                rule_token_counter
                    .expect_cap()
                    .once()
                    .return_const(PARAMS_MAX_CAPACITY);
                let metric = Arc::new(ParamsMetric {
                    rule_time_counter,
                    rule_token_counter,
                    ..Default::default()
                });

                let controller = gen_reject(rule, Some(metric));
                let token = controller.perform_checking(10110.to_string(), 20);
                assert!(token.is_pass());
                assert_eq!(30, old_qps.load(Ordering::SeqCst));
            }

            #[test]
            fn first_fill_token() {
                let rule = Arc::new(Rule {
                    resource: "abc".into(),
                    metric_type: MetricType::QPS,
                    control_strategy: ControlStrategy::Reject,
                    threshold: 100,
                    duration_in_sec: 1,
                    burst_count: 10,
                    ..Default::default()
                });
                let curr_time = utils::curr_time_millis();
                let last_add_token_time = Arc::new(AtomicU64::new(curr_time - 1001));
                let mut rule_time_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_time_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(Some(Arc::clone(&last_add_token_time)));
                rule_time_counter
                    .expect_cap()
                    .once()
                    .return_const(PARAMS_MAX_CAPACITY);
                let mut rule_token_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_token_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(None);
                rule_token_counter
                    .expect_cap()
                    .once()
                    .return_const(PARAMS_MAX_CAPACITY);
                let metric = Arc::new(ParamsMetric {
                    rule_time_counter,
                    rule_token_counter,
                    ..Default::default()
                });

                let controller = gen_reject(rule, Some(metric));
                utils::sleep_for_ms(10);
                let token = controller.perform_checking(10110.to_string(), 20);
                assert!(token.is_pass());
                assert!(last_add_token_time.load(Ordering::SeqCst) > curr_time);
            }

            #[test]
            fn refill_token() {
                let rule = Arc::new(Rule {
                    resource: "abc".into(),
                    metric_type: MetricType::QPS,
                    control_strategy: ControlStrategy::Reject,
                    threshold: 100,
                    duration_in_sec: 1,
                    burst_count: 10,
                    ..Default::default()
                });
                let old_qps = Arc::new(AtomicU64::new(50));
                let curr_time = utils::curr_time_millis();
                let last_add_token_time = Arc::new(AtomicU64::new(curr_time - 1001));
                let mut rule_time_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_time_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(Some(Arc::clone(&last_add_token_time)));
                rule_time_counter
                    .expect_cap()
                    .once()
                    .return_const(PARAMS_MAX_CAPACITY);
                let mut rule_token_counter: MockCounter<ParamKey> = MockCounter::new();
                rule_token_counter
                    .expect_add_if_absent()
                    .once()
                    .return_const(Some(Arc::clone(&old_qps)));
                rule_token_counter
                    .expect_cap()
                    .once()
                    .return_const(PARAMS_MAX_CAPACITY);
                let metric = Arc::new(ParamsMetric {
                    rule_time_counter,
                    rule_token_counter,
                    ..Default::default()
                });

                let controller = gen_reject(rule, Some(metric));
                utils::sleep_for_ms(10);
                let token = controller.perform_checking(10110.to_string(), 20);
                assert!(token.is_pass());
                assert!(last_add_token_time.load(Ordering::SeqCst) > curr_time);
                assert!(old_qps.load(Ordering::SeqCst) > 30);
            }
        }
    }

    mod throttling {
        use super::*;

        #[test]
        fn check_qps() {
            let rule = Arc::new(Rule {
                resource: "abc".into(),
                metric_type: MetricType::QPS,
                control_strategy: ControlStrategy::Throttling,
                threshold: 100,
                duration_in_sec: 1,
                max_queueing_time_ms: 10,
                ..Default::default()
            });
            let curr_time = utils::curr_time_millis();
            let last_add_token_time = Arc::new(AtomicU64::new(curr_time - 201));
            let mut rule_time_counter: MockCounter<ParamKey> = MockCounter::new();
            rule_time_counter
                .expect_add_if_absent()
                .once()
                .return_const(Some(Arc::clone(&last_add_token_time)));
            rule_time_counter
                .expect_cap()
                .return_const(CONCURRENCY_MAX_COUNT);
            let rule_token_counter: MockCounter<ParamKey> = MockCounter::new();
            let metric = Arc::new(ParamsMetric {
                rule_time_counter,
                rule_token_counter,
                ..Default::default()
            });
            let controller = gen_throttling(rule, Some(metric));
            let token = controller.perform_checking(10110.to_string(), 20);
            assert!(token.is_pass());
        }
    }
}
