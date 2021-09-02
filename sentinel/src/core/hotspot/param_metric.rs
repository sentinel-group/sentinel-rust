use time::PreciseTime;

use super::*;
use crate::base::ParamKey;
use std::any::Any;
use std::hash::Hash;
use std::sync::Arc;

pub const CONCURRENCY_MAX_COUNT: usize = 4000;
pub const PARAMS_CAPACITY_BASE: usize = 4000;
pub const PARAMS_MAX_CAPACITY: usize = 20000;

/// `ParamsMetric` carries real-time counters for frequent ("hot spot") parameters.
/// For each cache map, the key is the parameter value, while the value is the counter.
#[derive(Debug, Default)]
pub struct ParamsMetric<C = Counter>
where
    C: CounterTrait,
{
    /// rule_time_counter records the last added token timestamp.
    pub(crate) rule_time_counter: C,
    /// rule_token_counter records the number of tokens.
    pub(crate) rule_token_counter: C,
    /// concurrency_counter records the real-time concurrency.
    pub(crate) concurrency_counter: C,
}
