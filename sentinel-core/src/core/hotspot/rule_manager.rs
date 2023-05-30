use super::*;
use crate::{base::SentinelRule, logging, utils, Error, Result};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

// todo: this module is redundant as the flow control rule managers has been implemented in the `crate::core::flow`

/// ControllerGenfn represents the Traffic Controller generator function of a specific control behavior.
pub type ControllerGenfn<C = Counter> =
    dyn Send + Sync + Fn(Arc<Rule>, Option<Arc<ParamsMetric<C>>>) -> Arc<Controller>;

pub type ControllerMap = HashMap<String, Vec<Arc<Controller>>>;
pub type RuleMap = HashMap<String, HashSet<Arc<Rule>>>;

lazy_static! {
    // we only store the Specialization with `Counter`, the MockCounter is neglected here
    static ref GEN_FUN_MAP: RwLock<HashMap<ControlStrategy, Box<ControllerGenfn>>> = {
        // Initialize the traffic shaping controller generator map for existing control behaviors.

        let mut gen_fun_map:HashMap<ControlStrategy, Box<ControllerGenfn>> = HashMap::new();

        gen_fun_map.insert(
            ControlStrategy::Reject,
            Box::new(gen_reject::<Counter>),
        );

        gen_fun_map.insert(
            ControlStrategy::Throttling,
            Box::new(gen_throttling::<Counter>),
        );

        RwLock::new(gen_fun_map)
    };
    static ref CONTROLLER_MAP: RwLock<ControllerMap> = RwLock::new(HashMap::new());
    static ref RULE_MAP: Mutex<RuleMap> = Mutex::new(HashMap::new());
}

pub(super) use gen_fns::*;

mod gen_fns {
    use super::*;

    pub(in super::super) fn gen_reject<C: CounterTrait>(
        rule: Arc<Rule>,
        metric: Option<Arc<ParamsMetric<C>>>,
    ) -> Arc<Controller<C>> {
        let checker: Arc<Mutex<dyn Checker<C>>> = Arc::new(Mutex::new(RejectChecker::<C>::new()));
        let mut tsc = match metric {
            None => Controller::new(rule),
            Some(metric) => Controller::new_with_metric(rule, metric),
        };
        tsc.set_checker(Arc::clone(&checker));
        let tsc = Arc::new(tsc);
        let mut checker = checker.lock().unwrap();
        checker.set_owner(Arc::downgrade(&tsc));
        tsc
    }

    pub(in super::super) fn gen_throttling<C: CounterTrait>(
        rule: Arc<Rule>,
        metric: Option<Arc<ParamsMetric<C>>>,
    ) -> Arc<Controller<C>> {
        let checker: Arc<Mutex<dyn Checker<C>>> =
            Arc::new(Mutex::new(ThrottlingChecker::<C>::new()));
        let mut tsc = match metric {
            None => Controller::new(rule),
            Some(metric) => Controller::new_with_metric(rule, metric),
        };
        tsc.set_checker(Arc::clone(&checker));
        let tsc = Arc::new(tsc);
        let mut checker = checker.lock().unwrap();
        checker.set_owner(Arc::downgrade(&tsc));
        tsc
    }
}

pub fn get_traffic_controller_list_for(res: &String) -> Vec<Arc<Controller>> {
    CONTROLLER_MAP
        .read()
        .unwrap()
        .get(res)
        .unwrap_or(&Vec::new())
        .clone()
}

fn log_rule_update(map: &RuleMap) {
    if map.is_empty() {
        logging::info!("[HotspotRuleManager] Hotspot param flow rules were cleared")
    } else {
        logging::info!(
            "[HotspotRuleManager] Hotspot param flow rules were loaded: {:?}",
            map.values()
        )
    }
}

pub fn append_rule(rule: Arc<Rule>) -> bool {
    if RULE_MAP
        .lock()
        .unwrap()
        .get(&rule.resource)
        .unwrap_or(&HashSet::new())
        .contains(&rule)
    {
        return false;
    }
    match rule.is_valid() {
        Ok(_) => {
            RULE_MAP
                .lock()
                .unwrap()
                .entry(rule.resource.clone())
                .or_default()
                .insert(Arc::clone(&rule));
        }
        Err(err) => logging::warn!(
            "[Hot Spot append_rule] Ignoring invalid flow rule {:?}, reason: {:?}",
            rule,
            err
        ),
    }
    let mut placeholder = Vec::new();
    let new_tcs_of_res = build_resource_traffic_shaping_controller(
        &rule.resource,
        RULE_MAP.lock().unwrap().get(&rule.resource).unwrap(),
        CONTROLLER_MAP
            .write()
            .unwrap()
            .get_mut(&rule.resource)
            .unwrap_or(&mut placeholder),
    );
    if !new_tcs_of_res.is_empty() {
        CONTROLLER_MAP
            .write()
            .unwrap()
            .entry(rule.resource.clone())
            .or_default()
            .push(Arc::clone(&new_tcs_of_res[0]));
    }
    true
}

/// `load_rules` loads the given hotspot param flow rules to the rule manager, while all previous rules will be replaced.
/// The returned `bool` indicates whether do real load operation, if the rules is the same with previous rules, return false
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn load_rules(rules: Vec<Arc<Rule>>) -> bool {
    let mut rule_map: RuleMap = HashMap::new();
    for rule in rules {
        let entry = rule_map.entry(rule.resource.clone()).or_default();
        entry.insert(rule);
    }

    let mut global_rule_map = RULE_MAP.lock().unwrap();
    if *global_rule_map == rule_map {
        logging::info!(
            "[HotSpot] Load rules is the same with current rules, so ignore load operation."
        );
        return false;
    }
    // when rule_map is different with global one, update the global one
    // ignore invalid rules
    let mut valid_rules_map = HashMap::with_capacity(rule_map.len());
    for (res, rules) in &rule_map {
        let mut valid_rules = HashSet::new();
        for rule in rules {
            match rule.is_valid() {
                Ok(_) => {valid_rules.insert(Arc::clone(rule));},
                Err(err) => logging::warn!(
                    "[HotSpot onRuleUpdate] Ignoring invalid hotspot param flow rule {:?}, reason: {:?}",
                    rule,
                    err
                ),
            }
        }
        if !valid_rules.is_empty() {
            valid_rules_map.insert(res.clone(), valid_rules);
        }
    }

    let start = utils::curr_time_nanos();
    let mut controller_map = CONTROLLER_MAP.write().unwrap();
    let mut valid_controller_map = HashMap::with_capacity(valid_rules_map.len());

    // build controller_map according to valid rules
    for (res, rules) in valid_rules_map.iter() {
        let mut placeholder = Vec::new();
        let new_tcs_of_res = build_resource_traffic_shaping_controller(
            res,
            rules,
            controller_map.get_mut(res).unwrap_or(&mut placeholder),
        );
        if !new_tcs_of_res.is_empty() {
            valid_controller_map.insert(res.clone(), new_tcs_of_res);
        }
    }
    *controller_map = valid_controller_map;
    *global_rule_map = rule_map;
    drop(global_rule_map);
    drop(controller_map);
    logging::debug!(
        "[HotSpot load_rules] Time statistic(ns) for updating hotspot param flow rule, time cost {}",
        utils::curr_time_nanos() - start
    );

    log_rule_update(&valid_rules_map);
    true
}

/// `load_rules_of_resource` loads the given resource's flow rules to the rule manager, while all previous resource's rules will be replaced.
/// The first returned value indicates whether do real load operation, if the rules is the same with previous resource's rules, return false
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn load_rules_of_resource(res: &String, rules: Vec<Arc<Rule>>) -> Result<bool> {
    if res.is_empty() {
        return Err(Error::msg("empty resource"));
    }
    let rules: HashSet<_> = rules.into_iter().collect();
    let mut global_rule_map = RULE_MAP.lock().unwrap();
    let mut global_controller_map = CONTROLLER_MAP.write().unwrap();
    // clear resource rules
    if rules.is_empty() {
        global_rule_map.remove(res);
        global_controller_map.remove(res);
        logging::info!("[HotSpot] clear resource level rules, resource {}", res);
        return Ok(true);
    }
    // load resource level rules
    if global_rule_map.get(res).unwrap_or(&HashSet::new()) == &rules {
        logging::info!("[HotSpot] Load resource level rules is the same with current resource level rules, so ignore load operation.");
        return Ok(false);
    }

    let mut valid_res_rules = HashSet::with_capacity(res.len());
    for rule in &rules {
        match rule.is_valid() {
            Ok(_) => {
                valid_res_rules.insert(Arc::clone(rule));
            }
            Err(err) => logging::warn!(
                "[HotSpot load_rules_of_resource] Ignoring invalid flow rule {:?}, reason: {:?}",
                rule,
                err
            ),
        }
    }
    // the `res` related rules changes, have to update
    let start = utils::curr_time_nanos();
    let mut placeholder = Vec::new();
    let old_res_tcs = global_controller_map
        .get_mut(res)
        .unwrap_or(&mut placeholder);

    let valid_res_rules_string = format!("{:?}", &valid_res_rules);
    let new_res_tcs = build_resource_traffic_shaping_controller(res, &valid_res_rules, old_res_tcs);

    if new_res_tcs.is_empty() {
        global_controller_map.remove(res);
    } else {
        global_controller_map.insert(res.clone(), new_res_tcs);
    }

    global_rule_map.insert(res.clone(), rules);
    logging::debug!(
        "[HotSpot load_rules_of_resource] Time statistic(ns) for updating hotspot param flow rule, timeCost: {}",
        utils::curr_time_nanos() - start
    );
    logging::info!(
        "[HotSpot] load resource level hotspot param rules, resource: {}, valid_res_rules: {}",
        res,
        valid_res_rules_string
    );

    Ok(true)
}

/// `get_rules` returns all the rules in `CONTROLLER_MAP`
// This func acquires the locks on global `CONTROLLER_MAP`,
// please release your lock on it before calling this func
pub fn get_rules() -> Vec<Arc<Rule>> {
    let mut rules = Vec::new();
    let controller_map = CONTROLLER_MAP.read().unwrap();
    for (_, controllers) in controller_map.iter() {
        for c in controllers {
            rules.push(Arc::clone(c.rule()));
        }
    }
    rules
}

/// `get_rules_of_resource` returns specific resource's rules
// This func acquires the lock on global `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn get_rules_of_resource(res: &String) -> Vec<Arc<Rule>> {
    let controller_map = CONTROLLER_MAP.read().unwrap();
    let placeholder = Vec::new();
    let controllers = controller_map.get(res).unwrap_or(&placeholder);
    let mut rules = Vec::with_capacity(controllers.len());
    for c in controllers {
        rules.push(Arc::clone(c.rule()));
    }
    rules
}

/// clear_rules clears all the rules in hotspot param flow module.
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn clear_rules() {
    RULE_MAP.lock().unwrap().clear();
    CONTROLLER_MAP.write().unwrap().clear();
}

/// `clear_rules_of_resource` clears resource level rules in hotspot param flow module.
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn clear_rules_of_resource(res: &String) {
    RULE_MAP.lock().unwrap().remove(res);
    CONTROLLER_MAP.write().unwrap().remove(res);
}

/// `set_traffic_shaping_generator` sets the traffic controller generator for the given CalculateStrategy and ControlStrategy.
/// Note that modifying the generator of default control strategy is not allowed.
/// It is type safe
// This func acquires the lock on global `GEN_FUN_MAP`,
// please release your lock on it before calling this func
pub fn set_traffic_shaping_generator(
    control_strategy: ControlStrategy,
    generator: Box<ControllerGenfn>,
) -> Result<()> {
    match control_strategy {
        ControlStrategy::Custom(_) => {
            GEN_FUN_MAP
                .write()
                .unwrap()
                .insert(control_strategy, generator);
            Ok(())
        }
        _ => Err(Error::msg(
            "Default control behaviors are not allowed to be modified.",
        )),
    }
}

// This func acquires the lock on global `GEN_FUN_MAP`,
// please release your lock on it before calling this func
pub fn remove_traffic_shaping_generator(control_strategy: ControlStrategy) -> Result<()> {
    match control_strategy {
        ControlStrategy::Custom(_) => {
            GEN_FUN_MAP.write().unwrap().remove(&control_strategy);
            Ok(())
        }
        _ => Err(Error::msg(
            "Default control behaviors are not allowed to be removed.",
        )),
    }
}

fn calculate_reuse_index_for(r: &Arc<Rule>, old_res_tcs: &[Arc<Controller>]) -> (usize, usize) {
    // the index of equivalent rule in old traffic shaping controller slice
    let mut eq_idx = usize::MAX;
    // the index of statistic reusable rule in old traffic shaping controller slice
    let mut reuse_stat_idx = usize::MAX;

    for (idx, old_tc) in old_res_tcs.iter().enumerate() {
        let old_rule = old_tc.rule();
        if old_rule == r {
            // break if there is equivalent rule
            eq_idx = idx;
            break;
        }
        // search the index of first stat reusable rule
        if reuse_stat_idx == usize::MAX && old_rule.is_stat_reusable(r) {
            reuse_stat_idx = idx;
        }
    }
    (eq_idx, reuse_stat_idx)
}

/// build_resource_traffic_shaping_controller builds Controller slice from rules. the resource of rules must be equals to res
pub fn build_resource_traffic_shaping_controller(
    res: &String,
    rules_of_res: &HashSet<Arc<Rule>>,
    old_res_tcs: &mut Vec<Arc<Controller>>,
) -> Vec<Arc<Controller>> {
    let mut new_res_tcs = Vec::with_capacity(rules_of_res.len());
    for rule in rules_of_res {
        if res != &rule.resource {
            logging::error!("unmatched resource name expect: {}, actual: {}. Unmatched resource name in flow::build_resource_traffic_shaping_controller(), rule: {:?}", res, rule.resource, rule);
            continue;
        }
        let (eq_idx, reuse_stat_idx) = calculate_reuse_index_for(rule, old_res_tcs);

        // First check equals scenario
        if eq_idx != usize::MAX {
            // reuse the old tc
            let eq_old_tc = Arc::clone(&old_res_tcs[eq_idx]);
            new_res_tcs.push(eq_old_tc);
            // remove old tc from old_res_tcs
            old_res_tcs.remove(eq_idx);
            continue;
        }

        let gen_fun_map = GEN_FUN_MAP.read().unwrap();
        let generator = gen_fun_map.get(&rule.control_strategy);

        if generator.is_none() {
            logging::error!("[FlowRuleManager build_resource_traffic_shaping_controller] Unsupported flow control strategy. Ignoring the rule due to unsupported control behavior in flow::build_resource_traffic_shaping_controller(), rule: {}",  rule);
            continue;
        }
        let generator = generator.unwrap();

        let tc = {
            if reuse_stat_idx != usize::MAX {
                generator(
                    Arc::clone(rule),
                    Some(Arc::clone(old_res_tcs[reuse_stat_idx].metric())),
                )
            } else {
                generator(Arc::clone(rule), None)
            }
        };

        if reuse_stat_idx != usize::MAX {
            // remove old tc from old_res_tcs
            old_res_tcs.remove(reuse_stat_idx);
        }
        new_res_tcs.push(tc);
    }
    new_res_tcs
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn gen_without_metric() {
        let mut specific_items = HashMap::new();
        specific_items.insert(100.to_string(), 100);
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 110,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items,
            ..Default::default()
        });
        let gen_fun_map = GEN_FUN_MAP.read().unwrap();
        let generator = gen_fun_map.get(&ControlStrategy::Reject);
        let generator = generator.unwrap();
        let tc = generator(Arc::clone(&rule), None);
        assert!(Arc::ptr_eq(&rule, tc.rule()));
        assert_eq!(0, tc.param_index());
    }

    #[test]
    fn gen_with_metric() {
        let mut specific_items = HashMap::new();
        specific_items.insert(100.to_string(), 100);
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            metric_type: MetricType::QPS,
            control_strategy: ControlStrategy::Throttling,
            threshold: 110,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items,
            ..Default::default()
        });

        let capacity = std::cmp::min(
            PARAMS_MAX_CAPACITY,
            PARAMS_CAPACITY_BASE * rule.duration_in_sec as usize,
        );

        let metric = ParamsMetric {
            rule_time_counter: Counter::with_capacity(capacity),
            rule_token_counter: Counter::with_capacity(capacity),
            ..Default::default()
        };

        let gen_fun_map = GEN_FUN_MAP.read().unwrap();
        let generator = gen_fun_map.get(&ControlStrategy::Throttling);
        let generator = generator.unwrap();
        let tc = generator(Arc::clone(&rule), Some(Arc::new(metric)));
        assert!(Arc::ptr_eq(&rule, tc.rule()));
        assert_eq!(0, tc.param_index());
    }

    #[test]
    #[ignore]
    fn test_load_rules() {
        clear_rules();
        let mut specific_items = HashMap::new();
        specific_items.insert(String::from("sss"), 1);
        specific_items.insert(String::from("123"), 3);
        let rule = Arc::new(Rule {
            id: "1".into(),
            resource: "abc".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 100,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });

        let success = load_rules(vec![rule.clone()]);
        assert!(success);

        let success = load_rules(vec![rule]);
        assert!(!success);

        let controller_map = CONTROLLER_MAP.read().unwrap();
        let rule_map = RULE_MAP.lock().unwrap();

        assert_eq!(1, rule_map["abc"].len());
        assert_eq!(1, controller_map["abc"].len());
        drop(controller_map);
        drop(rule_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_load_rules_of_resource() {
        clear_rules();
        let mut specific_items = HashMap::new();
        specific_items.insert(String::from("sss"), 1);
        specific_items.insert(String::from("123"), 3);
        let r11 = Arc::new(Rule {
            id: "1".into(),
            resource: "abc1".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 100,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });
        let r12 = Arc::new(Rule {
            id: "2".into(),
            resource: "abc1".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 200,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });
        let r21 = Arc::new(Rule {
            id: "3".into(),
            resource: "abc2".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 100,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });
        let r22 = Arc::new(Rule {
            id: "4".into(),
            resource: "abc2".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 200,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items,
            ..Default::default()
        });

        let success = load_rules(vec![
            Arc::clone(&r11),
            Arc::clone(&r12),
            Arc::clone(&r21),
            Arc::clone(&r22),
        ]);
        assert!(success);

        let success = load_rules_of_resource(&"".into(), vec![Arc::clone(&r11), Arc::clone(&r12)]);
        assert!(success.is_err());

        let success =
            load_rules_of_resource(&"abc1".into(), vec![Arc::clone(&r11), Arc::clone(&r12)]);
        assert!(!success.unwrap());

        let success = load_rules_of_resource(&"abc1".into(), vec![]);
        assert!(success.unwrap());

        let controller_map = CONTROLLER_MAP.read().unwrap();
        let rule_map = RULE_MAP.lock().unwrap();

        assert_eq!(0, rule_map.get("abc1").unwrap_or(&HashSet::new()).len());
        assert_eq!(0, controller_map.get("abc1").unwrap_or(&Vec::new()).len());
        assert_eq!(2, rule_map.get("abc2").unwrap_or(&HashSet::new()).len());
        assert_eq!(2, controller_map.get("abc2").unwrap_or(&Vec::new()).len());
        drop(controller_map);
        drop(rule_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_clear_rules() {
        clear_rules();
        let mut specific_items = HashMap::new();
        specific_items.insert(String::from("sss"), 1);
        specific_items.insert(String::from("123"), 3);
        let r11 = Arc::new(Rule {
            id: "1".into(),
            resource: "abc1".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 100,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });
        let r12 = Arc::new(Rule {
            id: "2".into(),
            resource: "abc1".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 200,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });
        let r21 = Arc::new(Rule {
            id: "3".into(),
            resource: "abc2".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 100,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items: specific_items.clone(),
            ..Default::default()
        });
        let r22 = Arc::new(Rule {
            id: "4".into(),
            resource: "abc2".into(),
            metric_type: MetricType::Concurrency,
            control_strategy: ControlStrategy::Reject,
            threshold: 200,
            duration_in_sec: 1,
            burst_count: 10,
            specific_items,
            ..Default::default()
        });

        let success = load_rules(vec![r11, r12, r21, r22]);
        assert!(success);

        clear_rules_of_resource(&String::from("abc1"));

        assert_eq!(
            0,
            RULE_MAP
                .lock()
                .unwrap()
                .get("abc1")
                .unwrap_or(&HashSet::new())
                .len()
        );
        assert_eq!(
            0,
            CONTROLLER_MAP
                .read()
                .unwrap()
                .get("abc1")
                .unwrap_or(&Vec::new())
                .len()
        );
        assert_eq!(
            2,
            RULE_MAP
                .lock()
                .unwrap()
                .get("abc2")
                .unwrap_or(&HashSet::new())
                .len()
        );
        assert_eq!(
            2,
            CONTROLLER_MAP
                .read()
                .unwrap()
                .get("abc2")
                .unwrap_or(&Vec::new())
                .len()
        );
        clear_rules();
    }
}
