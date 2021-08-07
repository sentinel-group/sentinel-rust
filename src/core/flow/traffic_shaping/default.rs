use super::{Calculator, Checker, Controller, Rule};
use crate::core::base::{BlockType, MetricEvent, StatNode, TokenResult};
use std::sync::{Arc, Mutex, Weak};

/// Provide a determined threshold
#[derive(Debug)]
pub struct DirectCalculator {
    owner: Weak<Controller>,
    threshold: f64,
}

impl DirectCalculator {
    pub fn new(owner: Weak<Controller>, threshold: f64) -> Self {
        DirectCalculator { owner, threshold }
    }
}

impl Calculator for DirectCalculator {
    fn get_owner(&self) -> &Weak<Controller> {
        &self.owner
    }

    fn set_owner(&mut self, owner: Weak<Controller>) {
        self.owner = owner;
    }

    fn calculate_allowed_threshold(&self, _batch_count: u32, _flag: i32) -> f64 {
        self.threshold
    }
}

/// Directly reject
#[derive(Debug)]
pub struct RejectChecker {
    owner: Weak<Controller>,
    rule: Arc<Rule>,
}

impl RejectChecker {
    pub fn new(owner: Weak<Controller>, rule: Arc<Rule>) -> Self {
        RejectChecker { owner, rule }
    }
}

impl Checker for RejectChecker {
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
        let owner = self.owner.upgrade().unwrap();
        let read_only_metric = owner.bound_stat().read_only_metric();
        let cur_count = read_only_metric.sum(MetricEvent::Pass) as f64;
        if cur_count + batch_count as f64 > threshold {
            TokenResult::new_blocked_with_cause(
                BlockType::Flow,
                "flow reject check blocked".into(),
                self.rule.clone(),
                Arc::new(cur_count),
            )
        } else {
            TokenResult::new_pass()
        }
    }
}
