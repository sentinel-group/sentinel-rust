use super::*;
use crate::{base::rule::SentinelRule, logging, utils, Error, Result};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

pub type BreakerGenFn =
    dyn Send + Sync + Fn(Arc<Rule>, Option<Arc<CounterLeapArray>>) -> Arc<dyn CircuitBreakerTrait>;

pub type RuleMap = HashMap<String, HashSet<Arc<Rule>>>;

lazy_static! {
    pub static ref GEN_FUN_MAP: RwLock<HashMap<BreakerStrategy, Box<BreakerGenFn>>> = {
        let mut gen_fun_map: HashMap<BreakerStrategy, Box<BreakerGenFn>> = HashMap::new();
        gen_fun_map.insert(
            BreakerStrategy::SlowRequestRatio,
            Box::new(gen_slow_request),
        );
        gen_fun_map.insert(BreakerStrategy::ErrorCount, Box::new(gen_error_count));
        gen_fun_map.insert(BreakerStrategy::ErrorRatio, Box::new(gen_error_ratio));
        RwLock::new(gen_fun_map)
    };
    pub static ref STATE_CHANGE_LISTERNERS: Mutex<Vec<Arc<dyn StateChangeListener>>> =
        Mutex::new(Vec::new());
    pub static ref BREAKER_MAP: RwLock<HashMap<String, Vec<Arc<dyn CircuitBreakerTrait>>>> =
        RwLock::new(HashMap::new());
    pub static ref CURRENT_RULES: Mutex<RuleMap> = Mutex::new(HashMap::new());
    pub static ref BREAKER_RULES: RwLock<RuleMap> = RwLock::new(HashMap::new());
}

pub fn state_change_listeners() -> &'static Mutex<Vec<Arc<dyn StateChangeListener>>> {
    &STATE_CHANGE_LISTERNERS
}

use gen_fns::*;
mod gen_fns {
    use super::*;

    pub(super) fn gen_slow_request(
        rule: Arc<Rule>,
        stat: Option<Arc<CounterLeapArray>>,
    ) -> Arc<dyn CircuitBreakerTrait> {
        match stat {
            Some(stat) => Arc::new(SlowRtBreaker::new_with_stat(rule, stat)),
            None => {
                logging::warn!("[CircuitBreakerTrait RuleManager] Expect to generate circuit breaker with reuse statistic, but fail to do type casting, expect: CounterLeapArray");
                Arc::new(SlowRtBreaker::new(rule))
            }
        }
    }

    pub(super) fn gen_error_count(
        rule: Arc<Rule>,
        stat: Option<Arc<CounterLeapArray>>,
    ) -> Arc<dyn CircuitBreakerTrait> {
        match stat {
            Some(stat) => Arc::new(ErrorCountBreaker::new_with_stat(rule, stat)),
            None => {
                logging::warn!("[CircuitBreakerTrait RuleManager] Expect to generate circuit breaker with reuse statistic, but fail to do type casting, expect: CounterLeapArray");
                Arc::new(ErrorCountBreaker::new(rule))
            }
        }
    }

    pub(super) fn gen_error_ratio(
        rule: Arc<Rule>,
        stat: Option<Arc<CounterLeapArray>>,
    ) -> Arc<dyn CircuitBreakerTrait> {
        match stat {
            Some(stat) => Arc::new(ErrorRatioBreaker::new_with_stat(rule, stat)),
            None => {
                logging::warn!("[CircuitBreakerTrait RuleManager] Expect to generate circuit breaker with reuse statistic, but fail to do type casting, expect: CounterLeapArray");
                Arc::new(ErrorRatioBreaker::new(rule))
            }
        }
    }
}

/// `get_rules_of_resource` returns specific resource's rules
// This func acquires read locks on global `BREAKER_RULES`,
// please release your write locks on them before calling this func
pub fn get_rules_of_resource(res: &String) -> Vec<Arc<Rule>> {
    let breaker_rules = BREAKER_RULES.read().unwrap();
    let placeholder = HashSet::new();
    let res_rules = breaker_rules.get(res).unwrap_or(&placeholder);
    let mut rules = Vec::with_capacity(res_rules.len());
    for r in res_rules {
        rules.push(Arc::clone(r));
    }
    rules
}

/// `get_rules` returns all the rules
// This func acquires read locks on global `BREAKER_RULES`,
// please release your write locks on them before calling this func
pub fn get_rules() -> Vec<Arc<Rule>> {
    let mut rules = Vec::new();
    let breaker_rules = BREAKER_RULES.read().unwrap();
    for res_rules in (*breaker_rules).values() {
        for r in res_rules {
            rules.push(Arc::clone(r));
        }
    }
    rules
}

/// `clear_rules` clear all the previous rules.
// This func acquires locks on global `BREAKER_RULES`, `CURRENT_RULES` and `BREAKER_MAP`,
// please release your locks on them before calling this func
pub fn clear_rules() {
    CURRENT_RULES.lock().unwrap().clear();
    BREAKER_RULES.write().unwrap().clear();
    BREAKER_MAP.write().unwrap().clear();
}

pub fn append_rule(rule: Arc<Rule>) -> bool {
    if CURRENT_RULES
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
            CURRENT_RULES
                .lock()
                .unwrap()
                .entry(rule.resource.clone())
                .or_default()
                .insert(Arc::clone(&rule));
            BREAKER_RULES
                .write()
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
    let new_tcs_of_res = build_resource_circuit_breaker(
        &rule.resource,
        BREAKER_RULES.read().unwrap().get(&rule.resource).unwrap(),
        BREAKER_MAP
            .write()
            .unwrap()
            .get_mut(&rule.resource)
            .unwrap_or(&mut placeholder),
    );
    if !new_tcs_of_res.is_empty() {
        BREAKER_MAP
            .write()
            .unwrap()
            .entry(rule.resource.clone())
            .or_default()
            .push(Arc::clone(&new_tcs_of_res[0]));
    }
    true
}

/// load_rules replaces old rules with the given circuit breaking rules.
/// returned `bool` indicate whether the internal map has been changed
// This func acquires locks on global `CURRENT_RULES`, `BREAKER_RULES` and `BREAKER_MAP`,
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

    let mut global_rule_map = CURRENT_RULES.lock().unwrap();
    if *global_rule_map == rule_map {
        logging::info!(
            "[CircuitBreakerTrait] Loaded rules is the same with current rules, so ignore load operation."
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
    let mut global_breaker_map = BREAKER_MAP.write().unwrap();
    let mut valid_breaker_map = HashMap::with_capacity(valid_rules_map.len());

    // build global_breaker_map according to valid rules
    for (res, rules) in valid_rules_map.iter() {
        let mut placeholder = Vec::new();
        let new_cbs_of_res = build_resource_circuit_breaker(
            res,
            rules,
            global_breaker_map.get_mut(res).unwrap_or(&mut placeholder),
        );
        if !new_cbs_of_res.is_empty() {
            valid_breaker_map.insert(res.clone(), new_cbs_of_res);
        }
    }

    if valid_rules_map.is_empty() {
        logging::info!("[Circuit Breaker] Circuit breaking rules were cleared")
    } else {
        logging::info!(
            "[Circuit Breaker] Circuit breaking rules were loaded: {:?}",
            valid_rules_map.values()
        )
    }

    *BREAKER_RULES.write().unwrap() = valid_rules_map;
    *global_breaker_map = valid_breaker_map;
    *global_rule_map = rule_map;
    drop(global_rule_map);
    drop(global_breaker_map);
    logging::debug!(
        "[CircuitBreakerTrait load_rules] Time statistic(ns) for updating flow rule, time cost {}",
        utils::curr_time_nanos() - start
    );

    true
}

/// load_rulesOfResource loads the given resource's circuitBreaker rules to the rule manager, while all previous resource's rules will be replaced.
/// the first returned value indicates whether do real load operation, if the rules is the same with previous resource's rules, return false
// This func acquires locks on global `CURRENT_RULES`, `BREAKER_RULES` and `BREAKER_MAP`,
// please release your locks on them before calling this func
pub fn load_rules_of_resource(res: &String, rules: Vec<Arc<Rule>>) -> Result<bool> {
    if res.is_empty() {
        return Err(Error::msg("empty resource"));
    }
    let rules: HashSet<_> = rules.into_iter().collect();
    let mut global_rule_map = CURRENT_RULES.lock().unwrap();
    let mut global_breaker_map = BREAKER_MAP.write().unwrap();
    // clear resource rules
    if rules.is_empty() {
        global_rule_map.remove(res);
        global_breaker_map.remove(res);
        BREAKER_RULES.write().unwrap().remove(res);
        logging::info!(
            "[CircuitBreakerTrait] clear resource level rules, resource {}",
            res
        );
        return Ok(true);
    }
    // load resource level rules
    if global_rule_map.get(res).unwrap_or(&HashSet::new()) == &rules {
        logging::info!("[CircuitBreakerTrait] Load resource level rules is the same with current resource level rules, so ignore load operation.");
        return Ok(false);
    }

    let mut valid_res_rules = HashSet::with_capacity(res.len());
    for rule in &rules {
        match rule.is_valid() {
            Ok(_) => {valid_res_rules.insert(Arc::clone(rule));},
            Err(err) => logging::warn!(
                "CircuitBreakerTrait onResourceRuleUpdate] Ignoring invalid circuitBreaker rule {:?}, reason: {:?}",
                rule,
                err
            ),
        }
    }
    // the `res` related rules changes, have to update
    let start = utils::curr_time_nanos();
    let mut placeholder = Vec::new();
    let old_res_tcs = global_breaker_map.get_mut(res).unwrap_or(&mut placeholder);

    let valid_res_rules_string = format!("{:?}", &valid_res_rules);
    let new_res_tcs = build_resource_circuit_breaker(res, &valid_res_rules, old_res_tcs);

    if new_res_tcs.is_empty() {
        global_breaker_map.remove(res);
        BREAKER_RULES.write().unwrap().remove(res);
    } else {
        global_breaker_map.insert(res.clone(), new_res_tcs);
        BREAKER_RULES
            .write()
            .unwrap()
            .insert(res.clone(), valid_res_rules);
    }

    global_rule_map.insert(res.clone(), rules);
    logging::debug!(
        "[CircuitBreakerTrait onResourceRuleUpdate] Time statistics(ns) for updating circuit breaker rule, timeCost: {}",
        utils::curr_time_nanos() - start
    );
    logging::info!(
        "[CircuitBreakerTrait] load resource level rules, resource: {}, valid_res_rules: {}",
        res,
        valid_res_rules_string
    );

    Ok(true)
}

// This func acquires read locks on global `BREAKER_MAP`,
// please release your write locks on them before calling this func
pub fn get_breakers_of_resource(resource: &String) -> Vec<Arc<dyn CircuitBreakerTrait>> {
    let breakers_map = BREAKER_MAP.read().unwrap();
    let placeholder = Vec::new();
    let res_cbs = breakers_map.get(resource).unwrap_or(&placeholder);
    let mut breakers = Vec::with_capacity(res_cbs.len());
    for b in res_cbs {
        breakers.push(Arc::clone(b));
    }
    breakers
}

/// register_state_change_listeners registers the global state change listener for all circuit breakers
pub fn register_state_change_listeners(mut listeners: Vec<Arc<dyn StateChangeListener>>) {
    if listeners.is_empty() {
        return;
    }
    STATE_CHANGE_LISTERNERS
        .lock()
        .unwrap()
        .append(&mut listeners);
}

/// clear_state_change_listeners clears the all StateChangeListener
pub fn clear_state_change_listeners() {
    STATE_CHANGE_LISTERNERS.lock().unwrap().clear();
}

/// set_circuit_breaker_generator sets the circuit breaker generator for the given strategy.
/// Note that modifying the generator of default strategies is not allowed.
pub fn set_circuit_breaker_generator(
    s: BreakerStrategy,
    generator: Box<BreakerGenFn>,
) -> Result<()> {
    match s {
        BreakerStrategy::Custom(_) => {
            GEN_FUN_MAP.write().unwrap().insert(s, generator);
            Ok(())
        }
        _ => Err(Error::msg(
            "Default circuit breakers are not allowed to be modified.",
        )),
    }
}

pub fn remove_circuit_breaker_generator(s: &BreakerStrategy) -> Result<()> {
    match s {
        BreakerStrategy::Custom(_) => {
            GEN_FUN_MAP.write().unwrap().remove(s);
            Ok(())
        }
        _ => Err(Error::msg(
            "Default circuit breakers are not allowed to be modified.",
        )),
    }
}

/// `clear_rules_of_resource` clears resource level rules in circuitBreaker module.
pub fn clear_rules_of_resource(res: &String) {
    BREAKER_RULES.write().unwrap().remove(res);
    CURRENT_RULES.lock().unwrap().remove(res);
    BREAKER_MAP.write().unwrap().remove(res);
}

pub fn calculate_reuse_index_for(
    r: &Arc<Rule>,
    old_res_cbs: &[Arc<dyn CircuitBreakerTrait>],
) -> (usize, usize) {
    // the index of equivalent rule in old circuit breaker slice
    let mut eq_idx = usize::MAX;
    // the index of statistic reusable rule in old circuit breaker slice
    let mut reuse_stat_idx = usize::MAX;

    for (idx, old_cb) in old_res_cbs.iter().enumerate() {
        let old_rule = old_cb.bound_rule();
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

/// build_resource_circuit_breaker builds CircuitBreakerTrait slice from rules. the resource of rules must be equals to res
pub fn build_resource_circuit_breaker(
    res: &String,
    rules_of_res: &HashSet<Arc<Rule>>,
    old_res_cbs: &mut Vec<Arc<dyn CircuitBreakerTrait>>,
) -> Vec<Arc<dyn CircuitBreakerTrait>> {
    let mut new_res_cbs = Vec::with_capacity(rules_of_res.len());
    for rule in rules_of_res {
        if res != &rule.resource {
            logging::error!("unmatched resource name expect: {}, actual: {}. Unmatched resource name in CircuitBreakerTrait::build_resource_circuit_breaker(), rule: {:?}", res, rule.resource, rule);
            continue;
        }

        let (eq_idx, reuse_stat_idx) = calculate_reuse_index_for(rule, old_res_cbs);

        // First check equals scenario
        if eq_idx != usize::MAX {
            // reuse the old cb
            let eq_old_cb = Arc::clone(&old_res_cbs[eq_idx]);
            new_res_cbs.push(eq_old_cb);
            // remove old cb from old_res_cbs
            old_res_cbs.remove(eq_idx);
            continue;
        }

        let gen_fun_map = GEN_FUN_MAP.read().unwrap();
        let generator = gen_fun_map.get(&rule.strategy);
        if generator.is_none() {
            logging::error!("[CircuitBreakerTrait build_resource_circuit_breaker] Ignoring the rule due to unsupported circuit breaking strategy, rule {:?}", rule);
            continue;
        }
        let generator = generator.unwrap();

        let cb = {
            if reuse_stat_idx != usize::MAX {
                generator(
                    rule.clone(),
                    Some(Arc::clone(old_res_cbs[reuse_stat_idx].stat())),
                )
            } else {
                generator(rule.clone(), None)
            }
        };

        if reuse_stat_idx != usize::MAX {
            // remove old cb from old_res_tcs
            old_res_cbs.remove(reuse_stat_idx);
        }
        new_res_cbs.push(cb);
    }
    new_res_cbs
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic(expected = "Default circuit breakers are not allowed to be modified.")]
    fn illegal_set() {
        set_circuit_breaker_generator(
            BreakerStrategy::SlowRequestRatio,
            Box::new(
                |rule: Arc<Rule>,
                 _: Option<Arc<CounterLeapArray>>|
                 -> Arc<dyn CircuitBreakerTrait> {
                    Arc::new(SlowRtBreaker::new(rule))
                },
            ),
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Default circuit breakers are not allowed to be modified.")]
    fn illegal_remove() {
        remove_circuit_breaker_generator(&BreakerStrategy::SlowRequestRatio).unwrap();
    }

    #[test]
    #[ignore]
    fn set_and_remove_generator() {
        clear_rules();
        let key = BreakerStrategy::Custom(1);
        set_circuit_breaker_generator(
            key,
            Box::new(
                |rule: Arc<Rule>,
                 _: Option<Arc<CounterLeapArray>>|
                 -> Arc<dyn CircuitBreakerTrait> {
                    Arc::new(SlowRtBreaker::new(rule))
                },
            ),
        )
        .unwrap();
        let resource = String::from("test-customized-cb");
        load_rules(vec![Arc::new(Rule {
            resource: resource.clone(),
            strategy: key,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 0.3,
            ..Default::default()
        })]);

        let breaker_map = BREAKER_MAP.write().unwrap();

        assert!(GEN_FUN_MAP.read().unwrap().contains_key(&key));
        assert!(!breaker_map[&resource].is_empty());
        remove_circuit_breaker_generator(&key).unwrap();
        assert!(!GEN_FUN_MAP.read().unwrap().contains_key(&key));
        drop(breaker_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_load_rules_valid() {
        clear_rules();
        let r0 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let r1 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 0.3,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 10.0,
            ..Default::default()
        });
        let sucess = load_rules(vec![Arc::clone(&r0), Arc::clone(&r1), Arc::clone(&r2)]);
        assert!(sucess);
        let breaker_map = BREAKER_MAP.read().unwrap();
        let _b2 = &breaker_map["abc"][1];
        assert_eq!(breaker_map.len(), 1);
        assert_eq!(breaker_map["abc"].len(), 3);
        drop(breaker_map);

        let r3 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let r4 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 100,
            min_request_amount: 25,
            stat_interval_ms: 1000,
            threshold: 0.5,
            ..Default::default()
        });
        let r5 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 100,
            threshold: 10.0,
            ..Default::default()
        });
        let r6 = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1100,
            threshold: 10.0,
            ..Default::default()
        });

        let sucess = load_rules(vec![
            Arc::clone(&r3),
            Arc::clone(&r4),
            Arc::clone(&r5),
            Arc::clone(&r6),
        ]);
        assert!(sucess);
        let breaker_map = BREAKER_MAP.read().unwrap();
        let _b2 = &breaker_map["abc"][1];
        assert_eq!(breaker_map.len(), 1);
        assert_eq!(breaker_map["abc"].len(), 4);
        drop(breaker_map);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_load_rules_same() {
        clear_rules();
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let success = load_rules(vec![Arc::clone(&rule)]);
        assert!(success);
        let success = load_rules(vec![rule]);
        assert!(!success);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_load_rules_of_resource_invalid() {
        clear_rules();
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            ..Default::default()
        });
        let success = load_rules_of_resource(&"".into(), vec![rule]);
        assert!(success.is_err());
        assert_eq!(0, get_rules().len());
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_load_rules_of_resource() {
        clear_rules();
        let r0 = Arc::new(Rule {
            resource: "abc1".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 0.3,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc2".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 10.0,
            ..Default::default()
        });
        let success =
            load_rules_of_resource(&"abc1".into(), vec![Arc::clone(&r0), Arc::clone(&r1)]);
        assert!(success.unwrap());
        let success = load_rules_of_resource(&"abc2".into(), vec![Arc::clone(&r2)]);
        assert!(success.unwrap());
        let breaker_map = BREAKER_MAP.read().unwrap();
        let breaker_rules = BREAKER_RULES.read().unwrap();
        let current_rules = CURRENT_RULES.lock().unwrap();
        assert_eq!(2, breaker_map["abc1"].len());
        assert_eq!(2, breaker_rules["abc1"].len());
        assert_eq!(2, current_rules["abc1"].len());
        assert_eq!(1, breaker_map["abc2"].len());
        assert_eq!(1, breaker_rules["abc2"].len());
        assert_eq!(1, current_rules["abc2"].len());

        drop(breaker_map);
        drop(breaker_rules);
        drop(current_rules);

        let success =
            load_rules_of_resource(&"abc1".into(), vec![Arc::clone(&r0), Arc::clone(&r1)]);
        assert!(!success.unwrap());
        assert_eq!(2, BREAKER_MAP.read().unwrap()["abc1"].len());
        assert_eq!(2, BREAKER_RULES.read().unwrap()["abc1"].len());
        assert_eq!(2, CURRENT_RULES.lock().unwrap()["abc1"].len());

        let success = load_rules_of_resource(&"abc1".into(), Vec::new());
        assert!(success.unwrap());
        assert!(!BREAKER_MAP.read().unwrap().contains_key("abc1"));
        assert!(!BREAKER_RULES.read().unwrap().contains_key("abc1"));
        assert!(!CURRENT_RULES.lock().unwrap().contains_key("abc1"));

        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_get_rules() {
        clear_rules();
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let success = load_rules(vec![Arc::clone(&rule)]);
        assert!(success);
        let rules = get_rules();
        assert_eq!(1, rules.len());
        assert_eq!(rule.resource, rules[0].resource);
        assert_eq!(rule.strategy, rules[0].strategy);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_get_breakers_of_resource() {
        clear_rules();
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let success = load_rules(vec![Arc::clone(&rule)]);
        assert!(success);
        let breakers = get_breakers_of_resource(&rule.resource);
        assert_eq!(1, breakers.len());
        assert_eq!(breakers[0].bound_rule(), &rule);
        clear_rules();
    }

    #[test]
    #[ignore]
    fn test_clear_rules_of_resource() {
        clear_rules();
        let r0 = Arc::new(Rule {
            resource: "abc1".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            max_allowed_rt_ms: 20,
            threshold: 0.1,
            ..Default::default()
        });
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 0.3,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc2".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 1000,
            min_request_amount: 5,
            stat_interval_ms: 1000,
            threshold: 10.0,
            ..Default::default()
        });
        let success = load_rules(vec![r0, r1, r2]);
        assert!(success);

        clear_rules_of_resource(&"abc1".into());
        let breaker_map = BREAKER_MAP.read().unwrap();
        let breaker_rules = BREAKER_RULES.read().unwrap();
        let current_rules = CURRENT_RULES.lock().unwrap();
        assert_eq!(0, breaker_map.get("abc1").unwrap_or(&Vec::new()).len());
        assert_eq!(
            0,
            breaker_rules.get("abc1").unwrap_or(&HashSet::new()).len()
        );
        assert_eq!(
            0,
            current_rules.get("abc1").unwrap_or(&HashSet::new()).len()
        );
        assert_eq!(1, breaker_map["abc2"].len());
        assert_eq!(1, breaker_rules["abc2"].len());
        assert_eq!(1, current_rules["abc2"].len());
        drop(breaker_map);
        drop(breaker_rules);
        drop(current_rules);

        clear_rules();
    }
}
