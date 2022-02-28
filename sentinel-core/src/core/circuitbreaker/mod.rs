//! `circuitbreaker` module implements the circuit breaker pattern, which provides
//! stability and prevents cascading failures in distributed systems.
//!
//! Sentinel circuit breaker module supports three strategies:
//!
//!  1. SlowRequestRatio: the ratio of slow response time entry(entry's response time is great than max slow response time) exceeds the threshold. The following entry to resource will be broken.
//!                       In SlowRequestRatio strategy, user must set max response time.
//!
//!  2. ErrorRatio: the ratio of error entry exceeds the threshold. The following entry to resource will be broken.
//!  
//!  3. ErrorCount: the number of error entry exceeds the threshold. The following entry to resource will be broken.
//!
//! Sentinel converts each circuit breaking Rule into a CircuitBreaker. Each CircuitBreaker has its own statistical structure.
//!
//! Sentinel circuit breaker is implemented based on state machines. There are three states:
//!
//!  1. Closed: all entries could pass checking.
//!
//!  2. Open: the circuit breaker is broken, all entries are blocked. After retry timeout, circuit breaker switches state to Half-Open and allows one entry to probe whether the resource returns to its expected state.
//!
//!  3. Half-Open: the circuit breaker is in a temporary state of probing, only one entry is allowed to access resource, others are blocked.
//!
//! Sentinel circuit breaker provides the listeners with trait `StateChangeListener` to observe events of state changes.

pub mod breaker;
pub mod rule;
pub mod rule_manager;
pub mod slot;
pub mod stat_slot;

pub use breaker::*;
pub use rule::*;
pub use rule_manager::*;
pub use slot::*;
pub use stat_slot::*;
