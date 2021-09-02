use super::*;
use crate::{base::SentinelRule, logging, utils};
use crate::{Error, Result};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock, Weak};

pub type RuleMap = HashMap<String, Vec<Arc<Rule>>>;

lazy_static! {
    static ref RULE_MAP: RwLock<RuleMap> = RwLock::new(RuleMap::new());
    static ref CURRENT_RULES: Mutex<RuleMap> = Mutex::new(RuleMap::new());
}

/// `get_rules` returns all the rules in the global `RULE_MAP`
// This func acquires a read lock on global `RULE_MAP`,
// please release the lock before calling this func
pub fn get_rules() -> Vec<Arc<Rule>> {
    let rule_map = RULE_MAP.read().unwrap();
    let mut rules = Vec::with_capacity(rule_map.len());
    for r in rule_map.values() {
        rules.append(&mut r.clone());
    }
    rules
}

// `get_rules_of_resource` returns specific resource's rules
// This func acquires a read lock on global `RULE_MAP`,
// please release the lock before calling this func
pub fn get_rules_of_resource(res: &String) -> Vec<Arc<Rule>> {
    let mut placeholder = Vec::new();
    let rule_map = RULE_MAP.read().unwrap();
    let res_rules = rule_map.get(res).unwrap_or(&mut placeholder);
    let mut rules = res_rules.clone();
    rules
}

/// `load_rules` loads given isolation rules to the rule manager, while all previous rules will be replaced.
// This func acquires the locks on global `CURRENT_RULES` and `RULE_MAP`,
// please release the locks before calling this func
pub fn load_rules(rules: Vec<Arc<Rule>>) {
    let mut res_rules_map = RuleMap::new();
    for rule in rules {
        let mut val = res_rules_map
            .entry(rule.resource.clone())
            .or_insert(Vec::new());
        val.push(rule);
    }
    let mut current_rules = CURRENT_RULES.lock().unwrap();
    if &*current_rules == &res_rules_map {
        logging::info!(
            "[Isolation] Load rules is the same with current rules, so ignore load operation."
        );
        return;
    }

    // when rule_map is different with global one, update the global one
    // ignore invalid rules
    let mut valid_res_rule_map = RuleMap::with_capacity(res_rules_map.len());
    for (res, rules) in &res_rules_map {
        let mut valid_res_rules = Vec::with_capacity(rules.len());
        for rule in rules {
            match rule.is_valid() {
                Ok(_) => valid_res_rules.push(Arc::clone(&rule)),
                Err(err) => logging::warn!(
                    "[Isolation load_rules] Ignoring invalid flow rule {:?}, reason: {:?}",
                    rule,
                    err
                ),
            }
        }
        if valid_res_rules.len() > 0 {
            valid_res_rule_map.insert(res.clone(), valid_res_rules);
        }
    }

    let start = utils::curr_time_nanos();
    let mut rule_map = RULE_MAP.write().unwrap();
    *rule_map = valid_res_rule_map;
    *current_rules = res_rules_map;

    logging::debug!(
        "[Isolation load_rules] Time statistic(ns) for updating isolation rule, timeCost {:?}",
        utils::curr_time_nanos() - start
    );
    logging::info!(
        "[SystemRuleManager] Isolation rules loaded, rules {:?}",
        rule_map
    );
}

/// `load_rules` loads the given resource's isolation rules to the rule manager, while all previous resource's rules will be replaced.
// This func acquires the locks on global `CURRENT_RULES` and `RULE_MAP`,
// please release the locks before calling this func
pub fn load_rules_of_resource(res: &String, rules: Vec<Arc<Rule>>) -> Result<bool> {
    if res.len() == 0 {
        return Err(Error::msg("empty resource"));
    }

    if rules.len() == 0 {
        clear_rules_of_resource(res);
        logging::info!("[Isolation] clear resource level rules, resource {}", res);
        return Ok(true);
    }

    if CURRENT_RULES
        .lock()
        .unwrap()
        .get(res)
        .unwrap_or(&Vec::new())
        == &rules
    {
        logging::info!(
            "[Isolation] Load resource level rules is the same with current resource level rules, so ignore load operation."
        );
        return Ok(false);
    }

    // when rule_map is different with global one, update the global one
    // ignore invalid rules
    let mut valid_res_rules = Vec::with_capacity(rules.len());
    for rule in &rules {
        match rule.is_valid() {
            Ok(_) => valid_res_rules.push(Arc::clone(&rule)),
            Err(err) => logging::warn!(
                "[Isolation load_rules_of_resource] Ignoring invalid flow rule {:?}, reason: {:?}",
                rule,
                err
            ),
        }
    }

    let valid_res_rules_string = format!("{:?}", &valid_res_rules);
    let start = utils::curr_time_nanos();
    if valid_res_rules.len() == 0 {
        RULE_MAP.write().unwrap().remove(res);
    } else {
        RULE_MAP
            .write()
            .unwrap()
            .insert(res.clone(), valid_res_rules);
    }
    CURRENT_RULES.lock().unwrap().insert(res.clone(), rules);

    logging::debug!(
        "[Isolation load_rules] Time statistic(ns) for updating isolation rule, timeCost {:?}",
        utils::curr_time_nanos() - start
    );
    logging::info!(
        "[IsolationRuleManager] Isolation rules loaded, rules {}",
        valid_res_rules_string
    );
    return Ok(true);
}

/// `clear_rules` clear all the rules in isolation module
// This func acquires the locks on global `CURRENT_RULES` and `RULE_MAP`,
// please release the locks before calling this func
pub fn clear_rules() {
    CURRENT_RULES.lock().unwrap().clear();
    RULE_MAP.write().unwrap().clear();
}

/// ClearRulesOfResource clears resource level rules in isolation module.
// This func acquires the locks on global `CURRENT_RULES` and `RULE_MAP`,
// please release the locks before calling this func
pub fn clear_rules_of_resource(res: &String) {
    CURRENT_RULES.lock().unwrap().remove(res);
    RULE_MAP.write().unwrap().remove(res);
}

#[cfg(test)]
mod test {
    //! Some tests cannot run in parallel, since we cannot promise that
    //! the global data structs are not modified before assertion.
    use super::*;
    use crate::base::ReadStat;
    use crate::utils::AsAny;

    #[test]
    fn empty_rules() {
        let rules = get_rules();
        assert_eq!(0, rules.len());
    }

    #[test]
    #[ignore]
    fn several_rules() {
        // todo: in fact, here the threshold of "abc1" should be updated to 200,
        // instead of simply appending the rules
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 100,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 200,
            ..Default::default()
        });
        let r3 = Arc::new(Rule {
            threshold: 200,
            ..Default::default()
        });
        let r4 = Arc::new(Rule {
            resource: "abc3".into(),
            ..Default::default()
        });
        let r5 = Arc::new(Rule {
            resource: "abc3".into(),
            threshold: 10,
            ..Default::default()
        });
        load_rules(vec![
            Arc::clone(&r1),
            Arc::clone(&r2),
            r3,
            r4,
            Arc::clone(&r5),
        ]);
        let rule_map = RULE_MAP.read().unwrap();
        let current_rules = CURRENT_RULES.lock().unwrap();
        assert_eq!(2, rule_map.len());
        assert_eq!(2, rule_map["abc1"].len());
        assert_eq!(1, rule_map["abc3"].len());
        assert_eq!(2, current_rules["abc1"].len());
        assert_eq!(2, current_rules["abc3"].len());
        assert!(Arc::ptr_eq(&r1, &rule_map["abc1"][0]));
        assert!(Arc::ptr_eq(&r2, &rule_map["abc1"][1]));
        assert!(Arc::ptr_eq(&r5, &rule_map["abc3"][0]));
        drop(rule_map);
        drop(current_rules);

        clear_rules();
        assert_eq!(0, RULE_MAP.read().unwrap().len());
        assert_eq!(0, CURRENT_RULES.lock().unwrap().len());
    }

    #[test]
    #[ignore]
    #[should_panic(expected = "empty resource")]
    fn empty_resource() {
        let r1 = Arc::new(Rule {
            threshold: 100,
            ..Default::default()
        });
        let result = load_rules_of_resource(&"".into(), vec![r1]);
        assert_eq!(0, RULE_MAP.read().unwrap().len());
        result.unwrap();
    }

    #[test]
    #[ignore]
    fn several_rules_of_resources() {
        // todo: in fact, here the threshold of "abc1" should be updated to 200,
        // instead of simply appending the rules,
        // duplication should also be avoided
        let r1 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 100,
            ..Default::default()
        });
        let r2 = Arc::new(Rule {
            resource: "abc1".into(),
            threshold: 200,
            ..Default::default()
        });
        let r3 = Arc::new(Rule {
            resource: "abc3".into(),
            threshold: 10,
            ..Default::default()
        });
        let r4 = Arc::new(Rule {
            resource: "abc3".into(),
            threshold: 0,
            ..Default::default()
        });
        // todo: the consistency between resource and rule should be verified,
        // that is, rule of "abc3" cannot be loaded to "abc1"
        load_rules_of_resource(&"abc1".into(), vec![Arc::clone(&r1), Arc::clone(&r2)]);
        load_rules_of_resource(&"abc3".into(), vec![Arc::clone(&r3), Arc::clone(&r4)]);
        let rule_map = RULE_MAP.read().unwrap();
        let current_rules = CURRENT_RULES.lock().unwrap();
        assert_eq!(2, rule_map.len());
        assert_eq!(2, rule_map["abc1"].len());
        assert_eq!(1, rule_map["abc3"].len());
        assert_eq!(2, current_rules["abc1"].len());
        assert_eq!(2, current_rules["abc3"].len());
        assert!(Arc::ptr_eq(&r1, &rule_map["abc1"][0]));
        assert!(Arc::ptr_eq(&r2, &rule_map["abc1"][1]));
        assert!(Arc::ptr_eq(&r3, &rule_map["abc3"][0]));
        drop(rule_map);
        drop(current_rules);

        clear_rules_of_resource(&"abc1".into());
        assert_eq!(1, RULE_MAP.read().unwrap().len());
        assert_eq!(1, CURRENT_RULES.lock().unwrap().len());
        clear_rules_of_resource(&"abc3".into());
        assert_eq!(0, RULE_MAP.read().unwrap().len());
        assert_eq!(0, CURRENT_RULES.lock().unwrap().len());
    }
}
