use sentinel_rs;
use sentinel_rs::{base, flow, EntryBuilder};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// a "hello-world" example on async functions with explicit Sentinel entry builders
#[tokio::main]
async fn main() {
    // Init sentienl configurations
    sentinel_rs::init_default().unwrap_or_else(|err| sentinel_rs::logging::error!("{:?}", err));
    let resource_name = String::from("direct_reject_flow_control_example_on_tokio");

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
        handlers.push(tokio::spawn(async move {
            loop {
                let entry_builder = EntryBuilder::new(res_name.clone())
                    .with_traffic_type(base::TrafficType::Inbound);
                if let Ok(entry) = entry_builder.build() {
                    // Passed, wrap the logic here.
                    println!("{}: {}", sentinel_rs::utils::curr_time_millis(), "passed");
                    task().await;
                    // Be sure the entry is exited finally.
                    entry.read().unwrap().exit()
                } else {
                    sentinel_rs::utils::sleep_for_ms(100);
                }
            }
        }));
    }
    for h in handlers {
        h.await.expect("Couldn't join on the associated thread");
    }
}

async fn task() {
    sleep(Duration::from_millis(100)).await;
}
