//!  Circuit Breaker State Machine:
//!
//!                                switch to open based on rule
//!
//!             +-----------------------------------------------------------------------+
//!             |                                                                       |
//!             |                                                                       v
//!     +----------------+                   +----------------+      Probe      +----------------+
//!     |                |                   |                |<----------------|                |
//!     |                |   Probe succeed   |                |                 |                |
//!     |     Closed     |<------------------|    HalfOpen    |                 |      Open      |
//!     |                |                   |                |   Probe failed  |                |
//!     |                |                   |                +---------------->|                |
//!     +----------------+                   +----------------+                 +----------------+
//!

#![allow(clippy::wrong_self_convention)]

/// Error count
pub mod error_count;
/// Error ratio
pub mod error_ratio;
/// Slow round trip time
pub mod slow_request;
pub mod stat;

pub use error_count::*;
pub use error_ratio::*;
pub use slow_request::*;
pub use stat::*;

use super::*;
use crate::{
    base::{ContextPtr, EntryContext, SentinelEntry, Snapshot},
    logging,
    stat::MetricTrait,
    utils, Error, Result,
};
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

cfg_k8s! {
    use schemars::JsonSchema;
}

/// `BreakerStrategy` represents the strategy of circuit breaker.
/// Each strategy is associated with one rule type.
#[cfg_attr(feature = "ds_k8s", derive(JsonSchema))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum BreakerStrategy {
    /// `SlowRequestRatio` strategy changes the circuit breaker state based on slow request ratio
    SlowRequestRatio,
    /// `ErrorRatio` strategy changes the circuit breaker state based on error request ratio
    ErrorRatio,
    /// `ErrorCount` strategy changes the circuit breaker state based on error amount
    ErrorCount,
    #[serde(skip)]
    Custom(u8),
}

impl Default for BreakerStrategy {
    fn default() -> BreakerStrategy {
        BreakerStrategy::SlowRequestRatio
    }
}

/// States of Circuit Breaker State Machine
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum State {
    Closed,
    HalfOpen,
    Open,
}

impl Default for State {
    fn default() -> State {
        State::Closed
    }
}

impl State {}

/// `StateChangeListener` listens on the circuit breaker state change event
pub trait StateChangeListener: Sync + Send {
    /// on_transform_to_closed is triggered when circuit breaker state transformed to Closed.
    /// Argument rule is copy from circuit breaker's rule, any changes of rule don't take effect for circuit breaker
    /// Copying rule has a performance penalty and avoids invalid listeners as much as possible
    fn on_transform_to_closed(&self, prev: State, rule: Arc<Rule>);

    /// `on_transform_to_open` is triggered when circuit breaker state transformed to Open.
    /// The "snapshot" indicates the triggered value when the transformation occurs.
    /// Argument rule is copy from circuit breaker's rule, any changes of rule don't take effect for circuit breaker
    /// Copying rule has a performance penalty and avoids invalid listeners as much as possible
    fn on_transform_to_open(&self, prev: State, rule: Arc<Rule>, snapshot: Option<Arc<Snapshot>>);

    /// `on_transform_to_half_open` is triggered when circuit breaker state transformed to HalfOpen.
    /// Argument rule is copy from circuit breaker's rule, any changes of rule don't take effect for circuit breaker
    /// Copying rule has a performance penalty and avoids invalid listeners as much as possible
    fn on_transform_to_half_open(&self, prev: State, rule: Arc<Rule>);
}

/// `CircuitBreakerTrait` is the basic trait of circuit breaker
// todo: consider removing BreakerBase struct. Or keep it for simpler trait implementations
pub trait CircuitBreakerTrait: Send + Sync {
    /// `breaker` returns the associated inner breaker.

    fn breaker(&self) -> &BreakerBase;

    /// `stat` returns the associated statistic data structure.
    fn stat(&self) -> &Arc<CounterLeapArray>;

    /// `try_pass` acquires permission of an invocation only if it is available at the time of invocation.
    /// it checks circuit breaker based on state machine of circuit breaker.
    fn try_pass(&self, ctx: &EntryContext) -> bool {
        match self.current_state() {
            State::Closed => true,
            State::Open => {
                self.breaker().retry_timeout_arrived() && self.breaker().from_open_to_half_open(ctx)
            }
            State::HalfOpen => false,
        }
    }

    #[inline]
    fn next_retry_timestamp_ms(&self) -> u64 {
        self.breaker()
            .next_retry_timestamp_ms
            .load(Ordering::SeqCst)
    }

    /// `bound_rule` returns the associated circuit breaking rule.
    #[inline]
    fn bound_rule(&self) -> &Arc<Rule> {
        self.breaker().bound_rule()
    }

    #[inline]
    fn set_state(&self, state: State) {
        self.breaker().set_state(state);
    }

    /// `current_state` returns current state of the circuit breaker.
    #[inline]
    fn current_state(&self) -> State {
        self.breaker().current_state()
    }

    /// `on_request_complete` record a completed request with the given response time as well as error (if present),
    /// and handle state transformation of the circuit breaker.
    /// `on_request_complete` is called only when a passed invocation finished.
    // todo: Error propagation and handling
    fn on_request_complete(&self, rt: u64, error: &Option<Error>);

    /// the underlying metric should be with inner-mutability, thus, here we use `&self`
    fn reset_metric(&self) {
        for c in self.stat().all_counter() {
            c.value().reset()
        }
    }

    /// See doc for `BreakerBase`
    #[inline]
    fn from_closed_to_open(&self, snapshot: Arc<Snapshot>) -> bool {
        self.breaker().from_closed_to_open(snapshot)
    }

    #[inline]
    fn from_open_to_half_open(&self, ctx: &EntryContext) -> bool {
        self.breaker().from_open_to_half_open(ctx)
    }

    #[inline]
    fn from_half_open_to_open(&self, snapshot: Arc<Snapshot>) -> bool {
        self.breaker().from_half_open_to_open(snapshot)
    }

    #[inline]
    fn from_half_open_to_closed(&self) -> bool {
        self.breaker().from_half_open_to_closed()
    }
}

/// BreakerBase encompasses the common fields of circuit breaker.
#[derive(Debug)]
pub struct BreakerBase {
    rule: Arc<Rule>,
    /// retry_timeout_ms represents recovery timeout (in milliseconds) before the circuit breaker opens.
    /// During the open period, no requests are permitted until the timeout has elapsed.
    /// After that, the circuit breaker will transform to half-open state for trying a few "trial" requests.
    retry_timeout_ms: u32,
    /// next_retry_timestamp_ms is the time circuit breaker could probe
    next_retry_timestamp_ms: AtomicU64,
    /// state is the state machine of circuit breaker
    // todo: test `AtomicPtr`
    state: Arc<Mutex<State>>,
}

impl BreakerBase {
    pub fn bound_rule(&self) -> &Arc<Rule> {
        &self.rule
    }

    pub fn set_state(&self, state: State) {
        *self.state.lock().unwrap() = state;
    }

    pub fn current_state(&self) -> State {
        *self.state.lock().unwrap()
    }

    pub fn retry_timeout_arrived(&self) -> bool {
        utils::curr_time_millis() >= self.next_retry_timestamp_ms.load(Ordering::SeqCst)
    }

    pub fn update_next_retry_timestamp(&self) {
        self.next_retry_timestamp_ms.store(
            utils::curr_time_millis() + self.retry_timeout_ms as u64,
            Ordering::SeqCst,
        );
    }

    /// from_closed_to_open updates circuit breaker state machine from closed to open.
    /// Return true only if current goroutine successfully accomplished the transformation.
    pub fn from_closed_to_open(&self, snapshot: Arc<Snapshot>) -> bool {
        let mut state = self.state.lock().unwrap();
        if *state == State::Closed {
            *state = State::Open;
            self.update_next_retry_timestamp();
            let listeners = state_change_listeners().lock().unwrap();
            for listener in &*listeners {
                listener.on_transform_to_open(
                    State::Closed,
                    Arc::clone(&self.rule),
                    Some(Arc::clone(&snapshot)),
                );
            }

            #[cfg(feature = "exporter")]
            crate::exporter::add_state_change_counter(&self.rule.resource, "Closed", "Open");
            true
        } else {
            false
        }
    }

    /// from_open_to_half_open updates circuit breaker state machine from open to half-open.
    /// Return true only if current goroutine successfully accomplished the transformation.
    pub fn from_open_to_half_open(&self, ctx: &EntryContext) -> bool {
        let mut state = self.state.lock().unwrap();
        if *state == State::Open {
            *state = State::HalfOpen;
            let listeners = state_change_listeners().lock().unwrap();
            for listener in &*listeners {
                listener.on_transform_to_half_open(State::Open, Arc::clone(&self.rule));
            }
            let entry = ctx.entry();
            if let Some(entry) = entry {
                // add hook for entry exit
                // if the current circuit breaker performs the probe through this entry, but the entry was blocked,
                // this hook will guarantee current circuit breaker state machine will rollback to Open from Half-Open
                drop(state);
                let entry = entry.upgrade().unwrap();
                let rule = Arc::clone(&self.rule);
                let state = Arc::clone(&self.state);
                let mut entry = entry.write().unwrap();
                entry.when_exit(Box::new(
                    move |_entry: &SentinelEntry, ctx: ContextPtr| -> Result<()> {
                        let mut state = state.lock().unwrap();
                        let ctx = ctx.read().unwrap();
                        if ctx.is_blocked() && *state == State::HalfOpen {
                            *state = State::Open;
                            let listeners = state_change_listeners().lock().unwrap();
                            for listener in &*listeners {
                                listener.on_transform_to_open(
                                    State::HalfOpen,
                                    Arc::clone(&rule),
                                    Some(Arc::new(1.0)),
                                );
                            }
                        }
                        Ok(())
                    },
                ))
            } else {
                logging::error!(
                    "Entry is None in BreakerBase::from_open_to_half_open(), rule: {:?}",
                    self.rule,
                );
            }
            #[cfg(feature = "exporter")]
            crate::exporter::add_state_change_counter(&self.rule.resource, "Open", "HalfOpen");
            true
        } else {
            false
        }
    }

    /// from_half_open_to_open updates circuit breaker state machine from half-open to open.
    /// Return true only if current goroutine successfully accomplished the transformation.
    pub fn from_half_open_to_open(&self, snapshot: Arc<Snapshot>) -> bool {
        let mut state = self.state.lock().unwrap();
        if *state == State::HalfOpen {
            *state = State::Open;
            self.update_next_retry_timestamp();
            let listeners = state_change_listeners().lock().unwrap();
            for listener in &*listeners {
                listener.on_transform_to_open(
                    State::HalfOpen,
                    Arc::clone(&self.rule),
                    Some(Arc::clone(&snapshot)),
                );
            }

            #[cfg(feature = "exporter")]
            crate::exporter::add_state_change_counter(&self.rule.resource, "HalfOpen", "Open");
            true
        } else {
            false
        }
    }

    /// from_half_open_to_closed updates circuit breaker state machine from half-open to closed
    /// Return true only if current goroutine successfully accomplished the transformation.
    pub fn from_half_open_to_closed(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        if *state == State::HalfOpen {
            *state = State::Closed;
            let listeners = state_change_listeners().lock().unwrap();
            for listener in &*listeners {
                listener.on_transform_to_closed(State::HalfOpen, Arc::clone(&self.rule));
            }

            #[cfg(feature = "exporter")]
            crate::exporter::add_state_change_counter(&self.rule.resource, "HalfOpen", "Closed");
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
pub(crate) use test::MockCircuitBreaker;

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::base::{EntryContext, ResourceType, ResourceWrapper, SlotChain, TrafficType};
    use mockall::predicate::*;
    use mockall::*;
    use std::sync::RwLock;

    mock! {
        pub(crate) CircuitBreaker {}
        impl  CircuitBreakerTrait  for CircuitBreaker {
            fn breaker(&self) -> &BreakerBase;
            fn stat(&self) -> &Arc<CounterLeapArray>;
            fn bound_rule(&self) -> &Arc<Rule>;
            fn next_retry_timestamp_ms(&self)->u64;
            fn try_pass(&self, ctx: &EntryContext) -> bool;
            fn set_state(&self, state:State);
            fn current_state(&self) -> State;
            fn on_request_complete(&self, rt: u64, error: &Option<Error>);
            fn reset_metric(&self);
            fn from_closed_to_open(&self, snapshot: Arc<Snapshot>) -> bool;
            fn from_open_to_half_open(&self, ctx: &EntryContext) -> bool;
            fn from_half_open_to_open(&self, snapshot: Arc<Snapshot>) -> bool;
            fn from_half_open_to_closed(&self) -> bool;
        }
    }

    mock! {
        pub(crate) StateListener {}
        impl StateChangeListener for StateListener {
            fn on_transform_to_closed(&self, prev: State, rule: Arc<Rule>);
            fn on_transform_to_open(&self, prev: State, rule: Arc<Rule>, snapshot: Option<Arc<Snapshot>>);
            fn on_transform_to_half_open(&self, prev: State, rule: Arc<Rule>);
        }
    }

    #[test]
    #[ignore]
    fn custom_try_pass_closed() {
        // by default, the state of `breaker` is `State::Closed` (see the impl of `Default` trait on `State`)
        clear_state_change_listeners();
        let mut listener = MockStateListener::new();
        listener
            .expect_on_transform_to_half_open()
            .returning(|prev: State, rule: Arc<Rule>| {
                logging::debug!(
                    "transform to Half-Open, strategy: {:?}, previous state: {:?}",
                    rule.strategy,
                    prev
                );
            });
        register_state_change_listeners(vec![Arc::new(listener)]);
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::Custom(101),
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        });
        let breaker = SlowRtBreaker::new(Arc::clone(&rule));
        let token = breaker.try_pass(&EntryContext::new());
        clear_state_change_listeners();
        assert!(token);
    }

    #[test]
    #[ignore]
    fn custom_try_pass_probe() {
        clear_state_change_listeners();
        let mut listener = MockStateListener::new();
        listener
            .expect_on_transform_to_half_open()
            .returning(|prev: State, rule: Arc<Rule>| {
                logging::debug!(
                    "transform to Half-Open, strategy: {:?}, previous state: {:?}",
                    rule.strategy,
                    prev
                );
            });
        register_state_change_listeners(vec![Arc::new(listener)]);

        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::Custom(101),
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        });
        let breaker = SlowRtBreaker::new(rule);
        breaker.set_state(State::Open);
        let sc = Arc::new(SlotChain::new());
        let mut ctx = EntryContext::new();
        let res = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
        ctx.set_resource(res);
        let ctx = Arc::new(RwLock::new(ctx));
        let entry = Arc::new(RwLock::new(SentinelEntry::new(
            Arc::clone(&ctx),
            Arc::clone(&sc),
        )));
        ctx.write().unwrap().set_entry(Arc::downgrade(&entry));
        let token = breaker.try_pass(&ctx.read().unwrap());
        clear_state_change_listeners();
        assert!(token);
        assert_eq!(breaker.current_state(), State::HalfOpen);
    }

    #[test]
    #[ignore]
    fn slow_rt_try_pass_closed() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        });
        let breaker = SlowRtBreaker::new(Arc::clone(&rule));
        let token = breaker.try_pass(&EntryContext::new());
        assert!(token);
    }

    #[test]
    #[ignore]
    fn slow_rt_try_pass_probe() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        });
        let breaker = SlowRtBreaker::new(rule);
        breaker.set_state(State::Open);
        let sc = Arc::new(SlotChain::new());
        let mut ctx = EntryContext::new();
        let res = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
        ctx.set_resource(res);
        let ctx = Arc::new(RwLock::new(ctx));
        let entry = Arc::new(RwLock::new(SentinelEntry::new(
            Arc::clone(&ctx),
            Arc::clone(&sc),
        )));
        ctx.write().unwrap().set_entry(Arc::downgrade(&entry));
        let token = breaker.try_pass(&ctx.read().unwrap());
        assert!(token);
        assert_eq!(breaker.current_state(), State::HalfOpen);
    }

    #[test]
    #[ignore]
    fn slow_rt_on_request_complete() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::SlowRequestRatio,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            max_allowed_rt_ms: 50,
            threshold: 0.5,
            ..Default::default()
        });
        let breaker = SlowRtBreaker::new(rule);

        // less than min_request_amount
        breaker.on_request_complete(0, &None);
        assert_eq!(breaker.current_state(), State::Closed);

        // probe fails
        breaker.set_state(State::HalfOpen);
        breaker.on_request_complete(100, &None);
        assert_eq!(breaker.current_state(), State::Open);

        // probe succeeds
        breaker.set_state(State::HalfOpen);
        breaker.on_request_complete(10, &None);
        assert_eq!(breaker.current_state(), State::Closed);
    }

    #[test]
    #[ignore]
    fn error_ratio_try_pass_closed() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(Arc::clone(&rule));
        let token = breaker.try_pass(&EntryContext::new());
        assert!(token);
    }

    #[test]
    #[ignore]
    fn error_ratio_try_pass_probe() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(rule);
        breaker.set_state(State::Open);
        let sc = Arc::new(SlotChain::new());
        let mut ctx = EntryContext::new();
        let res = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
        ctx.set_resource(res);
        let ctx = Arc::new(RwLock::new(ctx));
        let entry = Arc::new(RwLock::new(SentinelEntry::new(
            Arc::clone(&ctx),
            Arc::clone(&sc),
        )));
        ctx.write().unwrap().set_entry(Arc::downgrade(&entry));
        let token = breaker.try_pass(&ctx.read().unwrap());
        assert!(token);
        assert_eq!(breaker.current_state(), State::HalfOpen);
    }

    #[test]
    #[ignore]
    fn error_ratio_on_request_complete() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorRatio,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 0.5,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(rule);

        // less than min_request_amount
        breaker.on_request_complete(0, &None);
        assert_eq!(breaker.current_state(), State::Closed);

        // probe fails
        breaker.set_state(State::HalfOpen);
        breaker.on_request_complete(0, &Some(Error::msg("error count")));
        assert_eq!(breaker.current_state(), State::Open);

        // probe succeeds
        breaker.set_state(State::HalfOpen);
        breaker.on_request_complete(0, &None);
        assert_eq!(breaker.current_state(), State::Closed);
    }

    #[test]
    #[ignore]
    fn error_count_try_pass_closed() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(Arc::clone(&rule));
        let token = breaker.try_pass(&EntryContext::new());
        assert!(token);
    }

    #[test]
    #[ignore]
    fn error_count_try_pass_probe() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(rule);
        breaker.set_state(State::Open);
        let sc = Arc::new(SlotChain::new());
        let mut ctx = EntryContext::new();
        let res = ResourceWrapper::new("abc".into(), ResourceType::Common, TrafficType::Inbound);
        ctx.set_resource(res);
        let ctx = Arc::new(RwLock::new(ctx));
        let entry = Arc::new(RwLock::new(SentinelEntry::new(
            Arc::clone(&ctx),
            Arc::clone(&sc),
        )));
        ctx.write().unwrap().set_entry(Arc::downgrade(&entry));
        let token = breaker.try_pass(&ctx.read().unwrap());
        assert!(token);
        assert_eq!(breaker.current_state(), State::HalfOpen);
    }

    #[test]
    #[ignore]
    fn error_count_on_request_complete() {
        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(rule);

        // less than min_request_amount
        breaker.on_request_complete(0, &None);
        assert_eq!(breaker.current_state(), State::Closed);

        // probe fails
        breaker.set_state(State::HalfOpen);
        breaker.on_request_complete(0, &Some(Error::msg("error count")));
        assert_eq!(breaker.current_state(), State::Open);

        // probe succeeds
        breaker.set_state(State::HalfOpen);
        breaker.on_request_complete(0, &None);
        assert_eq!(breaker.current_state(), State::Closed);
    }

    #[test]
    #[ignore]
    fn error_count_closed_to_open() {
        clear_state_change_listeners();
        let mut listener = MockStateListener::new();
        listener.expect_on_transform_to_open().once().returning(
            |prev: State, rule: Arc<Rule>, snapshot: Option<Arc<Snapshot>>| {
                logging::debug!(
                    "transform to Open, strategy: {:?}, previous state: {:?}, snapshot: {:?}",
                    rule.strategy,
                    prev,
                    snapshot
                );
            },
        );
        register_state_change_listeners(vec![Arc::new(listener)]);

        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(rule);
        let changed = breaker.from_closed_to_open(Arc::new(""));
        clear_state_change_listeners();
        assert!(changed);
    }

    #[test]
    #[ignore]
    fn error_count_half_open_to_open() {
        clear_state_change_listeners();
        let mut listener = MockStateListener::new();
        listener.expect_on_transform_to_open().once().returning(
            |prev: State, rule: Arc<Rule>, snapshot: Option<Arc<Snapshot>>| {
                logging::debug!(
                    "transform to Open, strategy: {:?}, previous state: {:?}, snapshot: {:?}",
                    rule.strategy,
                    prev,
                    snapshot
                );
            },
        );
        register_state_change_listeners(vec![Arc::new(listener)]);

        let rule = Arc::new(Rule {
            resource: "abc".into(),
            strategy: BreakerStrategy::ErrorCount,
            retry_timeout_ms: 3000,
            min_request_amount: 10,
            stat_interval_ms: 10000,
            threshold: 1.0,
            ..Default::default()
        });
        let breaker = ErrorCountBreaker::new(rule);
        breaker.set_state(State::HalfOpen);
        let changed = breaker.from_half_open_to_open(Arc::new(""));
        clear_state_change_listeners();
        assert!(changed);
        assert!(breaker.next_retry_timestamp_ms() > 0);
    }
}
