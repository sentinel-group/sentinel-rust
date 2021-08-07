//! Traffic Shaping Policy

/// Adaptive calculator
pub mod adaptive;
/// Default calculator and checker
pub mod default;
/// Throttling checker
pub mod throttling;
/// Warm Up calculator
pub mod warmup;

pub use adaptive::*;
pub use default::*;
pub use throttling::*;
pub use warmup::*;

use super::Rule;
use crate::core::base::{ReadStat, SentinelRule, StatNode, TokenResult, WriteStat};
use std::sync::{Arc, Mutex, Weak};

/// Traffic Shaping `Calculator` calculates the actual traffic shaping threshold
/// based on the threshold of rule and the traffic shaping strategy.
pub trait Calculator: Send + Sync + std::fmt::Debug {
    fn get_owner(&self) -> &Weak<Controller>;
    fn set_owner(&mut self, owner: Weak<Controller>);
    fn calculate_allowed_threshold(&self, batch_count: u32, flag: i32) -> f64;
}

/// Traffic Shaping `Checker` performs checking according to current metrics and the traffic
/// shaping strategy, then yield the token result.
pub trait Checker: Send + Sync + std::fmt::Debug {
    fn get_owner(&self) -> &Weak<Controller>;
    fn set_owner(&mut self, owner: Weak<Controller>);
    fn do_check(
        &self,
        stat_node: Option<Arc<dyn StatNode>>,
        batch_count: u32,
        threshold: f64,
    ) -> TokenResult;
}

/// StandaloneStat indicates the independent statistic for each Traffic Shaping Controller
#[derive(Debug)]
pub struct StandaloneStat {
    /// `reuse_global` indicates whether current standaloneStatistic reuse the current resource's global statistic
    reuse_global: bool,
    /// `read_only_metric` is the readonly metric statistic.
    /// if reuse_global is true, it would be the reused SlidingWindowMetric
    /// if reuse_global is false, it would be the BucketLeapArray
    read_only_metric: Arc<dyn ReadStat>,
    /// `write_only_metric` is the write only metric statistic.
    /// if reuse_global is true, it would be None
    /// if reuse_global is false, it would be the BucketLeapArray
    write_only_metric: Option<Arc<dyn WriteStat>>,
    // maybe here we should use Mutex inside Arc, incase of inner mutuability demands,
    // though our BucketLeapArray do not need it
}

impl StandaloneStat {
    pub fn new(
        reuse_global: bool,
        read_only_metric: Arc<dyn ReadStat>,
        write_only_metric: Option<Arc<dyn WriteStat>>,
    ) -> Self {
        StandaloneStat {
            reuse_global,
            read_only_metric,
            write_only_metric,
        }
    }

    pub fn reuse_global(&self) -> bool {
        self.reuse_global
    }

    pub fn read_only_metric(&self) -> &Arc<dyn ReadStat> {
        &self.read_only_metric
    }

    /// panics when reuse_global is true, meaning record metrics in global BucketLeapArray
    /// because it won't create one itself in this case
    pub fn write_only_metric(&self) -> Option<&Arc<dyn WriteStat>> {
        self.write_only_metric.as_ref()
    }
}

#[derive(Debug)]
pub struct Controller {
    calculator: Option<Arc<Mutex<dyn Calculator>>>,
    checker: Option<Arc<Mutex<dyn Checker>>>,
    rule: Arc<Rule>,
    // stat is the statistic of current Traffic Shaping Controller
    stat: Arc<StandaloneStat>,
}

impl Controller {
    pub fn new(rule: Arc<Rule>, stat: Arc<StandaloneStat>) -> Self {
        Controller {
            calculator: None,
            checker: None,
            rule,
            stat,
        }
    }

    pub fn rule(&self) -> &Arc<Rule> {
        &self.rule
    }

    pub fn get_checker(&self) -> &Arc<Mutex<dyn Checker>> {
        self.checker.as_ref().unwrap()
    }

    pub fn set_checker(&mut self, checker: Arc<Mutex<dyn Checker>>) {
        self.checker = Some(checker);
    }

    pub fn get_calculator(&self) -> &Arc<Mutex<dyn Calculator>> {
        self.calculator.as_ref().unwrap()
    }

    pub fn set_calculator(&mut self, calculator: Arc<Mutex<dyn Calculator>>) {
        self.calculator = Some(calculator);
    }

    pub fn bound_stat(&self) -> &Arc<StandaloneStat> {
        &self.stat
    }

    pub fn perform_checking(
        &self,
        res_stat: Arc<dyn StatNode>,
        batch_count: u32,
        flag: i32,
    ) -> TokenResult {
        let calculator = self.calculator.as_ref().unwrap();
        let calculator = calculator.lock().unwrap();
        let allowed_threshold = calculator.calculate_allowed_threshold(batch_count, flag);
        crate::metrics::set_resource_flow_threshold(self.rule.resource_name(), allowed_threshold);
        let checker = self.checker.as_ref().unwrap();
        let checker = checker.lock().unwrap();
        checker.do_check(Some(res_stat), batch_count, allowed_threshold)
    }
}
