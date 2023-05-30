#![allow(clippy::needless_update)]

use rand::prelude::*;
use sentinel_core::base::Snapshot;
use sentinel_core::circuitbreaker::{
    register_state_change_listeners, Rule, State, StateChangeListener,
};
use sentinel_core::utils::{curr_time_millis, sleep_for_ms};
use sentinel_macros::{circuitbreaker, flow};
use std::sync::Arc;

struct MyStateListener {}

impl StateChangeListener for MyStateListener {
    fn on_transform_to_closed(&self, prev: State, rule: Arc<Rule>) {
        println!(
            "rule.steategy: {:?}, From {:?} to Closed, time: {:?}\n",
            rule.strategy,
            prev,
            curr_time_millis()
        )
    }
    fn on_transform_to_open(&self, prev: State, rule: Arc<Rule>, snapshot: Option<Arc<Snapshot>>) {
        println!("rule.steategy: {:?}, From {:?} to Open, slwo request ratio snapshot: {:?}, time: {:?}\n", rule.strategy, prev, snapshot, curr_time_millis())
    }
    fn on_transform_to_half_open(&self, prev: State, rule: Arc<Rule>) {
        println!(
            "rule.steategy: {:?}, From {:?} to Half-Open, time: {:?}\n",
            rule.strategy,
            prev,
            curr_time_millis()
        )
    }
}

/// slow-request circuit breaking example with Sentinel attributes macros
fn main() {
    // Init sentienl configurations
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    let listeners: Vec<Arc<dyn StateChangeListener>> = vec![Arc::new(MyStateListener {})];
    register_state_change_listeners(listeners);

    let mut handlers = Vec::new();
    for _ in 0..20 {
        handlers.push(std::thread::spawn(move || {
            loop {
                task1().unwrap_or_else(|_| {
                    // blocked
                    sleep_for_ms(10);
                });
            }
        }));
    }
    for _ in 0..20 {
        handlers.push(std::thread::spawn(move || {
            loop {
                task2().unwrap_or_else(|_| {
                    // blocked
                    sleep_for_ms(10);
                });
            }
        }));
    }
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}

#[circuitbreaker(
    threshold = 0.2,
    max_allowed_rt_ms = 7,
    strategy = "SlowRequestRatio",
    retry_timeout_ms = 1000,
    min_request_amount = 30,
    stat_interval_ms = 1000,
    stat_sliding_window_bucket_count = 10
)]
fn task1() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    let uzi = (thread_rng().gen::<f32>() * 10.0).round() as u64;
    sleep_for_ms(uzi);
}

#[flow(
    traffic_type = "Outbound",
    calculate_strategy = "WarmUp",
    threshold = 10.0,
    warm_up_period_sec = 10,
    warm_up_cold_factor = 3
)]
fn task2() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(10);
}
