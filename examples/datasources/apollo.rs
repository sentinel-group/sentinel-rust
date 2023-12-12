#![allow(unreachable_code)]
use apollo_client::conf::{requests::WatchRequest, ApolloConfClientBuilder};
use sentinel_core::{
    base,
    datasource::{new_flow_rule_handler, rule_json_array_parser, ApolloDatasource},
    flow, EntryBuilder, Result,
};
use std::sync::Arc;
use tokio::{
    task::JoinHandle,
    time::{sleep, Duration},
};
use url::Url;

// An example on apollo config service data source.
// Run this example by following steps:
//  1. Set up apollo
//     (Quick start see https://github.com/apolloconfig/apollo-quick-start)
//  2. Run this example
//  3. Publish flow rule below at apollo-portal
//     key: flow-apollo-example
//     value:
//     [
//        {
//           "id":"1",
//           "resource":"task",
//           "ref_resource":"",
//           "calculate_strategy":"Direct",
//           "control_strategy":"Reject",
//           "relation_strategy":"Current",
//           "threshold":1.0,
//           "warm_up_period_sec":0,
//           "warm_up_cold_factor":0,
//           "max_queueing_time_ms":0,
//           "stat_interval_ms":0,
//           "low_mem_usage_threshold":0,
//           "high_mem_usage_threshold":0,
//           "mem_low_water_mark":0,
//           "mem_high_water_mark":0
//        }
//     ]
// You will find that QPS number is restricted to 10 at first. But after publish the new flow rule,
// it will be restricted to 1.
#[tokio::main]
async fn main() -> Result<()> {
    let handlers = basic_flow_example().await;
    // println!("{:?}", sentinel_core::flow::get_rules_of_resource(&"task".to_string()));

    // Create apollo client
    let client =
        ApolloConfClientBuilder::new_via_config_service(Url::parse("http://localhost:8080")?)?
            .build()?;

    // Request apollo notification api, and fetch configuration when notified.
    let watch_request = WatchRequest {
        app_id: "SampleApp".to_string(),
        namespace_names: vec![
            "application.properties".into(),
            "application.json".into(),
            "application.yml".into(),
        ],
        ..Default::default()
    };

    // Sleep 3 seconds and then read the apollo
    sentinel_core::utils::sleep_for_ms(3000);

    let property = "flow-apollo-example";
    // Create a data source and change the rule.
    let h = new_flow_rule_handler(rule_json_array_parser);
    let mut ds = ApolloDatasource::new(client, property.into(), watch_request, vec![h]);
    ds.initialize().await?;
    for h in handlers {
        h.await.expect("Couldn't join on the associated thread");
    }
    Ok(())
}

async fn basic_flow_example() -> Vec<JoinHandle<()>> {
    // Init sentienl configurations
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    let resource_name = String::from("task");
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
                    task().await;
                    // Be sure the entry is exited finally.
                    entry.exit()
                } else {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }));
    }
    handlers
}

// todo: Cannot sentinel-macros now. It will append rules,
// which is conflicts with the dynamic datasource
async fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep(Duration::from_millis(100)).await;
}
