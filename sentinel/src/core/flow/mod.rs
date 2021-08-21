//! Package flow implements the flow shaping control.
//!
//! flow module is based on QPS statistic metric
//!
//! The TrafficShapingController consists of two part: TrafficShapingCalculator and TrafficShapingChecker
//!
//!  1. TrafficShapingCalculator calculates the actual traffic shaping token threshold. Currently, Sentinel supports two token calculate strategy: Direct and WarmUp.
//!  2. TrafficShapingChecker performs checking logic according to current metrics and the traffic shaping strategy, then yield the token result. Currently, Sentinel supports two control behavior: Reject and Throttling.
//!
//! Besides, Sentinel supports customized TrafficShapingCalculator and TrafficShapingChecker. User could call function `set_traffic_shaping_generator()` to register customized TrafficShapingController and call function `remove_traffic_shaping_generator()` to unregister TrafficShapingController.
//! There are a few notes users need to be aware of:
//!
//!  1. The function both `set_traffic_shaping_generator()` and `remove_traffic_shaping_generator()` are not thread safe.
//!  2. Users can not override the Sentinel supported TrafficShapingController.
//!
//!

pub mod rule;
pub mod rule_manager;
pub mod slot;
pub mod standalone_stat_slot;
pub mod traffic_shaping;

pub use rule::*;
pub use rule_manager::*;
pub use slot::*;
pub use standalone_stat_slot::*;
pub use traffic_shaping::*;
