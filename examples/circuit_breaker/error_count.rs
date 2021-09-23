use anyhow::Error;
use rand::prelude::*;
use sentinel_rs::base::{Snapshot, TrafficType};
use sentinel_rs::circuitbreaker::{
    load_rules, register_state_change_listeners, BreakerStrategy, Rule, State, StateChangeListener,
};
use sentinel_rs::utils::{curr_time_millis, sleep_for_ms};
use sentinel_rs::EntryBuilder;
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
        println!(
            "rule.steategy: {:?}, From {:?} to Open, error ratio snapshot: {:?}, time: {:?}\n",
            rule.strategy,
            prev,
            snapshot,
            curr_time_millis()
        )
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

/// error-count circuit breaking example with explicit Sentinel entry builder
fn main() {
    // Init sentienl configurations
    sentinel_rs::init_default().unwrap_or_else(|err| sentinel_rs::logging::error!("{:?}", err));
    let listeners: Vec<Arc<dyn StateChangeListener>> = vec![Arc::new(MyStateListener {})];
    register_state_change_listeners(listeners);
    let resource_name = String::from("error_ratio_example");

    // Load sentinel rules
    load_rules(vec![Arc::new(Rule {
        resource: resource_name.clone(),
        threshold: 20.0,
        retry_timeout_ms: 1000,
        min_request_amount: 30,
        stat_interval_ms: 1000,
        stat_sliding_window_bucket_count: 10,
        strategy: BreakerStrategy::ErrorCount,
        ..Default::default()
    })]);

    let mut handlers = Vec::new();
    for _ in 0..20 {
        let res_name = resource_name.clone();
        handlers.push(std::thread::spawn(move || {
            loop {
                let entry_builder =
                    EntryBuilder::new(res_name.clone()).with_traffic_type(TrafficType::Inbound);
                if let Ok(entry) = entry_builder.build() {
                    // Passed, wrap the logic here.
                    println!("{}: passed", sentinel_rs::utils::curr_time_millis());
                    sleep_for_ms(10);
                    if thread_rng().gen::<f32>() > 0.6 {
                        entry.borrow().set_err(Error::msg("Example"));
                    }
                    // Be sure the entry is exited finally.
                    entry.borrow().exit()
                } else {
                    // Blocked. We could get the block reason from the BlockError.
                    sleep_for_ms(rand::random::<u64>() % 10);
                }
            }
        }));
    }
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}
