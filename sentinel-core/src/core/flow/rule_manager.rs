use super::*;
use crate::{
    core::{
        base,
        base::{nop_read_stat, nop_write_stat, ResourceType, SentinelRule, StatNode},
        config, stat,
        stat::ResourceNode,
    },
    logging, utils, Error, Result,
};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// ControllerGenfn represents the Traffic Controller generator function of a specific control behavior.
pub type ControllerGenfn =
    dyn Send + Sync + Fn(Arc<Rule>, Option<Arc<StandaloneStat>>) -> Result<Arc<Controller>>;

#[derive(Hash, PartialEq, Eq)]
pub struct ControllerGenKey {
    calculate_strategy: CalculateStrategy,
    control_strategy: ControlStrategy,
}

impl ControllerGenKey {
    pub fn new(calculate_strategy: CalculateStrategy, control_strategy: ControlStrategy) -> Self {
        ControllerGenKey {
            calculate_strategy,
            control_strategy,
        }
    }
}

/// ControllerMap represents the map storage for Controller.
pub type ControllerMap = HashMap<String, Vec<Arc<Controller>>>;
pub type RuleMap = HashMap<String, HashSet<Arc<Rule>>>;

lazy_static! {
    static ref GEN_FUN_MAP: RwLock<HashMap<ControllerGenKey, Box<ControllerGenfn>>> = {
        let mut gen_fun_map: HashMap<ControllerGenKey, Box<ControllerGenfn>> = HashMap::new();

        insert_flow_generator!(
            gen_fun_map,
            CalculateStrategy::Direct,
            ControlStrategy::Reject,
            DirectCalculator,
            RejectChecker
        );
        insert_flow_generator!(
            gen_fun_map,
            CalculateStrategy::Direct,
            ControlStrategy::Throttling,
            DirectCalculator,
            ThrottlingChecker
        );
        insert_flow_generator!(
            gen_fun_map,
            CalculateStrategy::WarmUp,
            ControlStrategy::Reject,
            WarmUpCalculator,
            RejectChecker
        );
        insert_flow_generator!(
            gen_fun_map,
            CalculateStrategy::WarmUp,
            ControlStrategy::Throttling,
            WarmUpCalculator,
            ThrottlingChecker
        );
        insert_flow_generator!(
            gen_fun_map,
            CalculateStrategy::MemoryAdaptive,
            ControlStrategy::Reject,
            MemoryAdaptiveCalculator,
            RejectChecker
        );
        insert_flow_generator!(
            gen_fun_map,
            CalculateStrategy::MemoryAdaptive,
            ControlStrategy::Throttling,
            MemoryAdaptiveCalculator,
            ThrottlingChecker
        );

        RwLock::new(gen_fun_map)
    };
    static ref CONTROLLER_MAP: Mutex<ControllerMap> = Mutex::new(HashMap::new());
    static ref NOP_STAT: Arc<StandaloneStat> = Arc::new(StandaloneStat::new(
        false,
        nop_read_stat(),
        Some(nop_write_stat())
    ));
    static ref RULE_MAP: Mutex<RuleMap> = Mutex::new(HashMap::new());
}

fn log_rule_update(map: &RuleMap) {
    if map.is_empty() {
        logging::info!("[FlowRuleManager] Flow rules were cleared")
    } else {
        logging::info!(
            "[FlowRuleManager] Flow rules were loaded: {:?}",
            map.values()
        )
    }
}

/// different from
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
            "[Flow load_rules] Ignoring invalid flow rule {:?}, reason: {:?}",
            rule,
            err
        ),
    }
    let mut placeholder = Vec::new();
    let new_tcs_of_res = build_resource_traffic_shaping_controller(
        &rule.resource,
        RULE_MAP.lock().unwrap().get(&rule.resource).unwrap(),
        CONTROLLER_MAP
            .lock()
            .unwrap()
            .get_mut(&rule.resource)
            .unwrap_or(&mut placeholder),
    );
    if !new_tcs_of_res.is_empty() {
        CONTROLLER_MAP
            .lock()
            .unwrap()
            .entry(rule.resource.clone())
            .or_default()
            .push(Arc::clone(&new_tcs_of_res[0]));
    }
    true
}

/// `load_rules` loads the given flow rules to the rule manager, while all previous rules will be replaced.
/// The returned `bool` indicates whether do real load operation, if the rules is the same with previous rules, return false
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn load_rules(rules: Vec<Arc<Rule>>) -> bool {
    let mut rule_map: RuleMap = HashMap::new();
    // todo: validate rules here,
    // neglect invalid rules,
    // instead of dealing with them in
    // `on_rule_update`
    for rule in rules {
        let entry = rule_map.entry(rule.resource.clone()).or_default();
        entry.insert(rule);
    }

    let mut global_rule_map = RULE_MAP.lock().unwrap();
    if *global_rule_map == rule_map {
        logging::info!(
            "[Flow] Load rules is the same with current rules, so ignore load operation."
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
                Ok(_) => {
                    valid_rules.insert(Arc::clone(rule));
                }
                Err(err) => logging::warn!(
                    "[Flow load_rules] Ignoring invalid flow rule {:?}, reason: {:?}",
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
    let mut controller_map = CONTROLLER_MAP.lock().unwrap();
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
        "[Flow load_rules] Time statistic(ns) for updating flow rule, time cost {}",
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
    let mut global_controller_map = CONTROLLER_MAP.lock().unwrap();
    // clear resource rules
    if rules.is_empty() {
        global_rule_map.remove(res);
        global_controller_map.remove(res);
        logging::info!("[Flow] clear resource level rules, resource {}", res);
        return Ok(true);
    }
    // load resource level rules
    if global_rule_map.get(res).unwrap_or(&HashSet::new()) == &rules {
        logging::info!("[Flow] Load resource level rules is the same with current resource level rules, so ignore load operation.");
        return Ok(false);
    }

    let mut valid_res_rules = HashSet::with_capacity(res.len());
    for rule in &rules {
        match rule.is_valid() {
            Ok(_) => {
                valid_res_rules.insert(Arc::clone(rule));
            }
            Err(err) => logging::warn!(
                "[Flow load_rules_of_resource] Ignoring invalid flow rule {:?}, reason: {:?}",
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
        "[Flow load_rules_of_resource] Time statistic(ns) for updating flow rule, timeCost: {}",
        utils::curr_time_nanos() - start
    );
    logging::info!(
        "[Flow] load resource level rules, resource: {}, valid_res_rules: {}",
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
    let controller_map = CONTROLLER_MAP.lock().unwrap();
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
    let controller_map = CONTROLLER_MAP.lock().unwrap();
    let placeholder = Vec::new();
    let controllers = controller_map.get(res).unwrap_or(&placeholder);
    let mut rules = Vec::with_capacity(controllers.len());
    for c in controllers {
        rules.push(Arc::clone(c.rule()));
    }
    rules
}

/// clear_rules clears all the rules in flow module.
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn clear_rules() {
    RULE_MAP.lock().unwrap().clear();
    CONTROLLER_MAP.lock().unwrap().clear();
}

/// `clear_rules_of_resource` clears resource level rules in flow module.
// This func acquires locks on global `RULE_MAP` and `CONTROLLER_MAP`,
// please release your locks on them before calling this func
pub fn clear_rules_of_resource(res: &String) {
    RULE_MAP.lock().unwrap().remove(res);
    CONTROLLER_MAP.lock().unwrap().remove(res);
}

// This func acquires the lock on global `CONTROLLER_MAP`,
// please release your lock on it before calling this func
pub fn get_traffic_controller_list_for(name: &String) -> Vec<Arc<Controller>> {
    let controller_map = CONTROLLER_MAP.lock().unwrap();
    let controllers = controller_map.get(name);
    match controllers {
        Some(controllers) => controllers.clone(),
        None => Vec::new(),
    }
}

/// `generate_stat_for` generates a `StandaloneStat` according to the rule,
/// it may generate a cloned pointer to the global `NOP_STAT`,
/// a new stat node with default global metrics
/// or a new stat node with new metrics.
fn generate_stat_for(rule: &Arc<Rule>) -> Result<Arc<StandaloneStat>> {
    if !rule.need_statistic() {
        return Ok(NOP_STAT.clone());
    }

    let interval_ms = rule.stat_interval_ms;

    let res_node: Arc<ResourceNode> = {
        if rule.relation_strategy == RelationStrategy::Associated {
            // use associated statistic
            stat::get_or_create_resource_node(&rule.ref_resource, &ResourceType::Common)
        } else {
            stat::get_or_create_resource_node(&rule.resource, &ResourceType::Common)
        }
    };

    if interval_ms == 0 || interval_ms == config::metric_stat_interval_ms() {
        // default case, use the resource's default statistic
        let metric = res_node.default_metric();
        let ret_stat = Arc::new(StandaloneStat::new(true, metric, None));
        return Ok(ret_stat);
    }

    let mut sample_count: u32 = 1;
    //calculate the sample count
    if interval_ms > config::global_stat_bucket_length_ms()
        && interval_ms < config::global_stat_interval_ms_total()
        && interval_ms % config::global_stat_bucket_length_ms() == 0
    {
        sample_count = interval_ms / config::global_stat_bucket_length_ms();
    }

    let validity = base::check_validity_for_reuse_statistic(
        sample_count,
        interval_ms,
        config::global_stat_sample_count_total(),
        config::global_stat_interval_ms_total(),
    );
    let _err = Error::msg(base::GLOBAL_STATISTIC_NON_REUSABLE_ERROR);
    match validity {
        Ok(_) => {
            let metric = res_node.generate_read_stat(sample_count, interval_ms)?;
            let ret_stat = Arc::new(StandaloneStat::new(true, metric, None));
            Ok(ret_stat)
        }
        Err(_err) => {
            logging::info!("[FlowRuleManager] Flow rule couldn't reuse global statistic and will generate independent statistic, rule: {:?}", rule);
            let write_stat = Arc::new(stat::BucketLeapArray::new(sample_count, interval_ms)?);
            let read_stat = Arc::new(stat::SlidingWindowMetric::new(
                sample_count,
                interval_ms,
                write_stat.clone(),
            )?);
            let res_stat = Arc::new(StandaloneStat::new(false, read_stat, Some(write_stat)));
            Ok(res_stat)
        }
    }
}

/// `set_traffic_shaping_generator` sets the traffic controller generator for the given CalculateStrategy and ControlStrategy.
/// Note that modifying the generator of default control strategy is not allowed.
/// it is type safe
// This func acquires the lock on global `GEN_FUN_MAP`,
// please release your lock on it before calling this func
pub fn set_traffic_shaping_generator(
    calculate_strategy: CalculateStrategy,
    control_strategy: ControlStrategy,
    generator: Box<ControllerGenfn>,
) -> Result<()> {
    match (calculate_strategy, control_strategy) {
        (CalculateStrategy::Custom(_), _) | (_, ControlStrategy::Custom(_)) => {
            GEN_FUN_MAP.write().unwrap().insert(
                ControllerGenKey::new(calculate_strategy, control_strategy),
                generator,
            );
            Ok(())
        }
        _ => Err(Error::msg(
            "Default control behaviors are not allowed to be modified.",
        )),
    }
}

// This func acquires the lock on global `GEN_FUN_MAP`,
// please release your lock on it before calling this func
pub fn remove_traffic_shaping_generator(
    calculate_strategy: CalculateStrategy,
    control_strategy: ControlStrategy,
) -> Result<()> {
    match (calculate_strategy, control_strategy) {
        (CalculateStrategy::Custom(_), _) | (_, ControlStrategy::Custom(_)) => {
            GEN_FUN_MAP
                .write()
                .unwrap()
                .remove(&ControllerGenKey::new(calculate_strategy, control_strategy));
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
        let key = ControllerGenKey::new(rule.calculate_strategy, rule.control_strategy);
        let generator = gen_fun_map.get(&key);

        if generator.is_none() {
            logging::error!("[FlowRuleManager build_resource_traffic_shaping_controller] Unsupported flow control strategy. Ignoring the rule due to unsupported control behavior in flow::build_resource_traffic_shaping_controller(), rule: {}",  rule);
            continue;
        }
        let generator = generator.unwrap();

        let tc = {
            if reuse_stat_idx != usize::MAX {
                generator(
                    Arc::clone(rule),
                    Some(Arc::clone(old_res_tcs[reuse_stat_idx].stat())),
                )
            } else {
                generator(Arc::clone(rule), None)
            }
        };

        if tc.is_err() {
            logging::error!("[FlowRuleManager build_resource_traffic_shaping_controller] Bad generated traffic controller. Ignoring the rule due to bad generated traffic controller in flow::build_resource_traffic_shaping_controller(), rule: {:?}, error: {:?}", rule, tc);
            continue;
        }
        let tc = tc.unwrap();
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
    //! Some tests cannot run in parallel, since we cannot promise that
    //! the global data structs are not modified before assertion.
    #![allow(clippy::vtable_address_comparisons)]

    use super::*;

    #[test]
    #[should_panic(expected = "Default control behaviors are not allowed to be modified.")]
    fn illegal_set() {
        set_traffic_shaping_generator(
            CalculateStrategy::Direct,
            ControlStrategy::Reject,
            Box::new(
                |_: Arc<Rule>, _: Option<Arc<StandaloneStat>>| -> Result<Arc<Controller>> {
                    let rule = Arc::new(Rule::default());
                    let stat = generate_stat_for(&rule).unwrap();
                    let tsc = Arc::new(Controller::new(rule, stat));
                    Ok(tsc)
                },
            ),
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Default control behaviors are not allowed to be removed.")]
    fn illegal_remove() {
        remove_traffic_shaping_generator(CalculateStrategy::Direct, ControlStrategy::Reject)
            .unwrap();
    }

    #[test]
    #[ignore]
    fn set_and_remove_generator() {
        clear_rules();
        const STRATEGY: u8 = 1;
        set_traffic_shaping_generator(
            CalculateStrategy::Custom(STRATEGY),
            ControlStrategy::Custom(STRATEGY),
            Box::new(
                |_: Arc<Rule>, _: Option<Arc<StandaloneStat>>| -> Result<Arc<Controller>> {
                    let rule = Arc::new(Rule::default());
                    let stat = generate_stat_for(&rule).unwrap();
                    let tsc = Arc::new(Controller::new(rule, stat));
                    Ok(tsc)
                },
            ),
        )
        .unwrap();
        let resource = String::from("test-customized-tc");
        load_rules(vec![Arc::new(Rule {
            threshold: 20.0,
            resource: resource.clone(),
            calculate_strategy: CalculateStrategy::Custom(STRATEGY),
            control_strategy: ControlStrategy::Custom(STRATEGY),
            ..Default::default()
        })]);
        let key = ControllerGenKey {
            calculate_strategy: CalculateStrategy::Custom(STRATEGY),
            control_strategy: ControlStrategy::Custom(STRATEGY),
        };

        let controller_map = CONTROLLER_MAP.lock().unwrap();

        assert!(GEN_FUN_MAP.read().unwrap().contains_key(&key));
        assert!(!controller_map[&resource].is_empty());
        remove_traffic_shaping_generator(
            CalculateStrategy::Custom(STRATEGY),
            ControlStrategy::Custom(STRATEGY),
        )
        .unwrap();
        assert!(!GEN_FUN_MAP.read().unwrap().contains_key(&key));
        drop(controller_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn get_rules1() {
        clear_rules();
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc2".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Throttling,
            max_queueing_time_ms: 10,
            stat_interval_ms: 1000,
            ..Default::default()
        });
        load_rules(vec![Arc::clone(&r1), Arc::clone(&r2)]);
        let rs = get_rules();

        if rs[0].resource == *"abc1" {
            // Arc<T> equals when inner T equals, even if they are different pointers
            assert_eq!(rs[0], r1);
            assert_eq!(rs[1], r2);
        } else {
            assert_eq!(rs[0], r2);
            assert_eq!(rs[1], r1);
        }
        clear_rules();
    }

    #[test]
    #[ignore]
    fn get_rules2() {
        clear_rules();
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc2".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Throttling,
            max_queueing_time_ms: 10,
            stat_interval_ms: 1000,
            ..Default::default()
        });
        load_rules(vec![r1.clone(), r2.clone()]);
        let rs = get_rules();

        if rs[0].resource == *"abc1" {
            // Arc<T> equals when inner T equals, even if they are different pointers
            assert_eq!(rs[0], r1);
            assert_eq!(rs[1], r2);
        } else {
            assert_eq!(rs[0], r2);
            assert_eq!(rs[1], r1);
        }

        let controller_map = CONTROLLER_MAP.lock().unwrap();

        assert_eq!(1, controller_map["abc2"].len());
        assert!(!controller_map["abc2"][0].stat().reuse_global());

        assert!(Arc::ptr_eq(
            controller_map["abc2"][0].stat().read_only_metric(),
            NOP_STAT.read_only_metric()
        ));
        assert!(Arc::ptr_eq(
            controller_map["abc2"][0]
                .stat()
                .write_only_metric()
                .unwrap(),
            NOP_STAT.write_only_metric().unwrap()
        ));
        drop(controller_map);
        clear_rules();
    }

    #[test]
    fn generate_stat_for_default_metric_stat() {
        let r1 = Arc::new(Rule {
            resource: "abc".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            ..Default::default()
        });
        let bound_stat = generate_stat_for(&r1).unwrap();
        assert!(bound_stat.reuse_global());

        let res_node = stat::get_resource_node(&String::from("abc")).unwrap();
        let stat = res_node.default_metric();
        assert!(Arc::ptr_eq(bound_stat.read_only_metric(), &stat));
    }

    #[test]
    fn generate_stat_for_reuse_global_stat() {
        let r1 = Arc::new(Rule {
            resource: "abc".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 5000,
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            ..Default::default()
        });
        let bound_stat = generate_stat_for(&r1).unwrap();
        assert!(bound_stat.reuse_global());
        assert!(bound_stat.write_only_metric().is_none());

        let res_node = stat::get_resource_node(&String::from("abc")).unwrap();
        let stat = res_node.default_metric();
        assert!(!Arc::ptr_eq(bound_stat.read_only_metric(), &stat));
    }

    #[test]
    fn generate_stat_for_standalone_stat() {
        let r1 = Arc::new(Rule {
            resource: "abc".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 50000,
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            ..Default::default()
        });

        let stat = generate_stat_for(&r1).unwrap();
        assert!(!stat.reuse_global());
        assert!(stat.write_only_metric().is_some());
    }

    #[test]
    #[ignore]
    fn build_controller1() {
        clear_rules();
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });

        let r2 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 200.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Throttling,
            max_queueing_time_ms: 10,
            ..Default::default()
        });

        let mut controller_map = CONTROLLER_MAP.lock().unwrap();
        assert_eq!(
            0,
            controller_map
                .entry(String::from("abc1"))
                .or_default()
                .len()
        );

        let mut placeholder = Vec::new();
        let mut set = HashSet::new();
        set.insert(Arc::clone(&r1));
        set.insert(Arc::clone(&r2));
        let tcs = build_resource_traffic_shaping_controller(
            &String::from("abc1"),
            &set,
            controller_map.get_mut("abc1").unwrap_or(&mut placeholder),
        );
        assert_eq!(2, tcs.len());
        assert!(&r1 == tcs[0].rule() || &r2 == tcs[0].rule());
        assert!(&r1 == tcs[1].rule() || &r2 == tcs[1].rule());
        drop(controller_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn build_controller2() {
        clear_rules();
        // use nop statistics because of no need statistics
        let r0 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Throttling,
            stat_interval_ms: 1000,
            ..Default::default()
        });
        // reuse resource node default leap array
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 100.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        });
        // reuse resource node default leap array
        let r2 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 200.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 2000,
            ..Default::default()
        });
        // reuse resource node default leap array
        let r3 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 300.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 5000,
            ..Default::default()
        });
        // use independent leap array because of too big interval
        let r4 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 400.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 50000,
            ..Default::default()
        });

        let s0 = generate_stat_for(&r0).unwrap();
        let fake_tc0 = Arc::new(Controller::new(Arc::clone(&r0), s0));
        let stat0 = fake_tc0.stat();
        assert!(Arc::ptr_eq(&NOP_STAT, stat0));
        assert!(!stat0.reuse_global());
        assert!(stat0.write_only_metric().is_some());

        let s1 = generate_stat_for(&r1).unwrap();
        let fake_tc1 = Arc::new(Controller::new(Arc::clone(&r1), s1));
        let stat1 = fake_tc1.stat();
        assert!(!Arc::ptr_eq(&NOP_STAT, stat1));
        assert!(stat1.reuse_global());
        assert!(stat1.write_only_metric().is_none());

        let s2 = generate_stat_for(&r2).unwrap();
        let fake_tc2 = Arc::new(Controller::new(Arc::clone(&r2), s2));
        let stat2 = fake_tc2.stat();
        assert!(!Arc::ptr_eq(&NOP_STAT, stat2));
        assert!(stat2.reuse_global());
        assert!(stat2.write_only_metric().is_none());

        let s3 = generate_stat_for(&r3).unwrap();
        let fake_tc3 = Arc::new(Controller::new(Arc::clone(&r3), s3));
        let stat3 = fake_tc3.stat();
        assert!(!Arc::ptr_eq(&NOP_STAT, stat3));
        assert!(stat3.reuse_global());
        assert!(stat3.write_only_metric().is_none());

        let s4 = generate_stat_for(&r4).unwrap();
        let fake_tc4 = Arc::new(Controller::new(Arc::clone(&r4), s4));
        let stat4 = fake_tc4.stat();
        assert!(!Arc::ptr_eq(&NOP_STAT, stat4));
        assert!(!stat4.reuse_global());
        assert!(stat4.write_only_metric().is_some());

        let mut controller_map = CONTROLLER_MAP.lock().unwrap();

        controller_map.insert(
            "abc1".into(),
            vec![
                Arc::clone(&fake_tc0),
                Arc::clone(&fake_tc1),
                Arc::clone(&fake_tc2),
                Arc::clone(&fake_tc3),
                Arc::clone(&fake_tc4),
            ],
        );
        assert_eq!(5, controller_map["abc1"].len());
        // reuse stat with rule 1
        let r12 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 300.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            stat_interval_ms: 1000,
            ..Default::default()
        });
        // can't reuse stat with rule 2, generate from resource's global statistic
        let r22 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 400.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 10000,
            ..Default::default()
        });
        // equals with rule 3
        let r32 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 300.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 5000,
            ..Default::default()
        });
        // reuse independent stat with rule 4
        let r42 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 4000.0,
            relation_strategy: RelationStrategy::Current,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            max_queueing_time_ms: 10,
            stat_interval_ms: 50000,
            ..Default::default()
        });

        let mut set = HashSet::new();
        set.insert(Arc::clone(&r12));
        set.insert(Arc::clone(&r22));
        set.insert(Arc::clone(&r32));
        set.insert(Arc::clone(&r42));
        let tcs = build_resource_traffic_shaping_controller(
            &String::from("abc1"),
            &set,
            controller_map.get_mut("abc1").unwrap(),
        );

        assert_eq!(4, tcs.len());

        // cannot automatically drop,
        // since the `controller_map` has not leave its scope
        // explicitly drop it or put the codes above in an individual `{}`
        drop(controller_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn load_resource_by_rule() {
        let r11 = Arc::new(Rule {
            resource: "abc1".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            threshold: 10.0,
            ..Default::default()
        });
        let r12 = Arc::new(Rule {
            resource: "abc1".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            threshold: 20.0,
            ..Default::default()
        });
        let r21 = Arc::new(Rule {
            resource: "abc2".into(),
            threshold: 10.0,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });
        let r22 = Arc::new(Rule {
            resource: "abc2".into(),
            threshold: 20.0,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });

        load_rules(vec![r11.clone(), r12.clone(), r21, r22]);

        let result = load_rules_of_resource(&String::from(""), vec![r11.clone(), r12.clone()]);
        assert!(result.is_err());

        let result = load_rules_of_resource(&String::from("abc1"), vec![r11, r12]);
        assert!(!result.unwrap());

        let result = load_rules_of_resource(&String::from("abc1"), vec![]);
        assert!(result.unwrap());

        let rule_map = RULE_MAP.lock().unwrap();
        let controller_map = CONTROLLER_MAP.lock().unwrap();

        assert_eq!(0, controller_map.get("abc1").unwrap_or(&Vec::new()).len());
        assert_eq!(0, rule_map.get("abc1").unwrap_or(&HashSet::new()).len());
        assert_eq!(2, controller_map["abc2"].len());
        assert_eq!(2, rule_map["abc2"].len());
    }

    #[test]
    #[ignore]
    fn clear_rules_by_resource() {
        let r11 = Arc::new(Rule {
            resource: "abc1".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            threshold: 10.0,
            ..Default::default()
        });
        let r12 = Arc::new(Rule {
            resource: "abc1".into(),
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            threshold: 20.0,
            ..Default::default()
        });
        let r21 = Arc::new(Rule {
            resource: "abc2".into(),
            threshold: 10.0,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });
        let r22 = Arc::new(Rule {
            resource: "abc2".into(),
            threshold: 20.0,
            calculate_strategy: CalculateStrategy::Direct,
            control_strategy: ControlStrategy::Reject,
            ..Default::default()
        });

        load_rules(vec![r11, r12, r21, r22]);
        clear_rules_of_resource(&String::from("abc1"));

        let rule_map = RULE_MAP.lock().unwrap();
        let controller_map = CONTROLLER_MAP.lock().unwrap();

        assert_eq!(0, controller_map.get("abc1").unwrap_or(&Vec::new()).len());
        assert_eq!(0, rule_map.get("abc1").unwrap_or(&HashSet::new()).len());
        assert_eq!(2, controller_map["abc2"].len());
        assert_eq!(2, rule_map["abc2"].len());
        drop(controller_map);
        drop(rule_map);
        clear_rules();
    }
}
