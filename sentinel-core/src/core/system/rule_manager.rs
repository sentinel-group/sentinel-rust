use super::*;
use crate::{base::SentinelRule, logging, utils};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

pub type RuleMap = HashMap<MetricType, HashSet<Arc<Rule>>>;

lazy_static! {
    static ref RULE_MAP: RwLock<RuleMap> = RwLock::new(RuleMap::new());
    static ref CURRENT_RULES: Mutex<Vec<Arc<Rule>>> = Mutex::new(Vec::new());
}

/// `get_rules` returns all the rules in the global `RULE_MAP`
// This func acquires a read lock on global `RULE_MAP`,
// please release the lock before calling this func
pub fn get_rules() -> Vec<Arc<Rule>> {
    let rule_map = RULE_MAP.read().unwrap();
    let mut rules: Vec<Arc<Rule>> = Vec::with_capacity(rule_map.len());
    for r in rule_map.values() {
        rules.append(&mut r.clone().into_iter().collect());
    }
    rules
}

pub fn append_rule(rule: Arc<Rule>) -> bool {
    if RULE_MAP
        .read()
        .unwrap()
        .get(&rule.metric_type)
        .unwrap_or(&HashSet::new())
        .contains(&rule)
    {
        return false;
    }

    match rule.is_valid() {
        Ok(_) => {
            RULE_MAP
                .write()
                .unwrap()
                .entry(rule.metric_type)
                .or_default()
                .insert(Arc::clone(&rule));
            CURRENT_RULES.lock().unwrap().push(rule);
        }
        Err(err) => logging::warn!(
            "[System append_rule] Ignoring invalid rule {:?}, reason: {:?}",
            rule,
            err
        ),
    };
    true
}

/// `load_rules` loads given system rules to the rule manager, while all previous rules will be replaced.
// This func acquires the lock on global `CURRENT_RULES`,
// please release the lock before calling this func
pub fn load_rules(rules: Vec<Arc<Rule>>) {
    let mut current_rules = CURRENT_RULES.lock().unwrap();
    if *current_rules == rules {
        logging::info!(
            "[System] Load rules is the same with current rules, so ignore load operation."
        );
        return;
    }

    // when rule_map is different with global one, update the global one
    // ignore invalid rules
    let m = build_rule_map(rules.clone());

    let start = utils::curr_time_nanos();
    let mut rule_map = RULE_MAP.write().unwrap();
    *rule_map = m;

    logging::debug!(
        "[System load_rules] Time statistic(ns) for updating system rule, timeCost {:?}",
        utils::curr_time_nanos() - start
    );
    logging::info!(
        "[SystemRuleManager] System rules loaded, rules {:?}",
        rule_map
    );
    *current_rules = rules;
}

/// `clear_rules` clear all the previous rules
// This func acquires the locks on global `CURRENT_RULES` and `RULE_MAP`,
// please release the locks before calling this func
pub fn clear_rules() {
    CURRENT_RULES.lock().unwrap().clear();
    RULE_MAP.write().unwrap().clear();
}

fn build_rule_map(rules: Vec<Arc<Rule>>) -> RuleMap {
    let mut m = RuleMap::new();
    for rule in rules {
        if let Err(err) = rule.is_valid() {
            logging::warn!(
                "[System build_rule_map] Ignoring invalid system rule, rule: {:?}, error: {:?}",
                rule,
                err
            );
            continue;
        }
        let value = m.entry(rule.metric_type).or_default();
        value.insert(rule);
    }
    m
}

#[cfg(test)]
mod test {
    //! Some tests cannot run in parallel, since we cannot promise that
    //! the global data structs are not modified before assertion.
    use super::*;

    #[test]
    fn empty_rules() {
        let rules = get_rules();
        assert_eq!(0, rules.len());
    }

    #[test]
    #[ignore]
    fn get_updated_rules() {
        let mut map = RuleMap::new();
        map.insert(MetricType::InboundQPS, HashSet::new());
        map.get_mut(&MetricType::InboundQPS)
            .unwrap()
            .insert(Arc::new(Rule {
                metric_type: MetricType::InboundQPS,
                threshold: 1.0,
                ..Default::default()
            }));
        map.insert(MetricType::Concurrency, HashSet::new());
        map.get_mut(&MetricType::Concurrency)
            .unwrap()
            .insert(Arc::new(Rule {
                metric_type: MetricType::Concurrency,
                threshold: 1.0,
                ..Default::default()
            }));

        let mut rule_map = RULE_MAP.write().unwrap();
        *rule_map = map.clone();
        drop(rule_map);
        let rules = get_rules();
        assert_eq!(2, rules.len());

        let rule = Arc::new(Rule {
            metric_type: MetricType::InboundQPS,
            threshold: 2.0,
            ..Default::default()
        });
        map.get_mut(&MetricType::InboundQPS).unwrap().insert(rule);
        let mut rule_map = RULE_MAP.write().unwrap();
        *rule_map = map;
        drop(rule_map);
        let rules = get_rules();
        assert_eq!(3, rules.len());

        clear_rules();
    }

    #[test]
    #[ignore]
    fn valid_system_rule() {
        let rules = vec![
            Arc::new(Rule {
                metric_type: MetricType::InboundQPS,
                threshold: 1.0,
                ..Default::default()
            }),
            Arc::new(Rule {
                metric_type: MetricType::Concurrency,
                threshold: 2.0,
                ..Default::default()
            }),
        ];
        load_rules(rules);
        assert_eq!(2, RULE_MAP.read().unwrap().len());
        clear_rules();
        assert_eq!(0, RULE_MAP.read().unwrap().len());
        assert_eq!(0, CURRENT_RULES.lock().unwrap().len());
    }

    #[test]
    fn invalid_build_map() {
        let rules = vec![Arc::new(Rule {
            metric_type: MetricType::InboundQPS,
            threshold: -1.0,
            ..Default::default()
        })];
        let map = build_rule_map(rules);
        assert_eq!(0, map.len());
    }

    #[test]
    fn valid_build_map() {
        let rules = vec![
            Arc::new(Rule {
                metric_type: MetricType::InboundQPS,
                threshold: 1.0,
                ..Default::default()
            }),
            Arc::new(Rule {
                metric_type: MetricType::Concurrency,
                threshold: 2.0,
                ..Default::default()
            }),
        ];
        let map = build_rule_map(rules);
        assert_eq!(2, map.len());
    }

    #[test]
    fn mix_build_map() {
        let rules = vec![
            Arc::new(Rule {
                metric_type: MetricType::InboundQPS,
                threshold: 1.0,
                ..Default::default()
            }),
            Arc::new(Rule {
                metric_type: MetricType::InboundQPS,
                threshold: 2.0,
                ..Default::default()
            }),
        ];
        let map = build_rule_map(rules);
        assert_eq!(1, map.len());
    }
}
