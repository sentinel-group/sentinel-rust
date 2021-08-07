use rand;
use sentinel;
use sentinel::utils::sleep_for_ms;
use sentinel::{
    core::{base, flow},
    EntryBuilder,
};
use std::sync::Arc;

#[test]
fn direct_reject() {
    // Init sentienl configurations
    sentinel::init_default().unwrap_or_else(|err| sentinel::logging::error!("{:?}", err));
    let resource_name = String::from("direct_reject_test");

    // Load sentinel rules
    flow::load_rules(vec![Arc::new(flow::Rule {
        resource: resource_name.clone(),
        threshold: 10.0,
        calculate_strategy: flow::CalculateStrategy::Direct,
        control_strategy: flow::ControlStrategy::Reject,
        ..Default::default()
    })]);
    let mut handlers = Vec::new();
    for _ in 0..20 {
        let res_name = resource_name.clone();
        handlers.push(std::thread::spawn(move || {
            loop {
                let entry_builder = EntryBuilder::new(res_name.clone())
                    .with_traffic_type(base::TrafficType::Inbound);
                if let Ok(entry) = entry_builder.build() {
                    // Passed, wrap the logic here.
                    println!("{}: {}", sentinel::utils::curr_time_millis(), "passed");
                    sleep_for_ms(rand::random::<u64>() % 10);
                    // Be sure the entry is exited finally.
                    entry.exit()
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
