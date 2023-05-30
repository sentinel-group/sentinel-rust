#![allow(unreachable_code)]
use etcd_rs::{Client, ClientConfig, PutRequest};
use sentinel_core::{
    base,
    datasource::{ds_etcdv3::Etcdv3DataSource, new_flow_rule_handler, rule_json_array_parser},
    flow, EntryBuilder, Result,
};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

// An example on etcd-v3 dynamic data source.
// Install etcd-v3, and run `etdc` in your terminal first. Then run this example.
// You will find that QPS number is restricted to 10 at first. But soon, it will be restricted to 1.
#[tokio::main]
async fn main() -> Result<()> {
    let handlers = basic_flow_example().await;

    // Create etcd client and put a key-value pair for new rule.
    let endpoints = vec!["http://127.0.0.1:2379".to_owned()];
    let client = Client::connect(ClientConfig {
        endpoints,
        auth: None,
        tls: None,
    })
    .await?;
    let key = "flow-etcdv3-example";

    {
        // Dynamically add a rule to the etcd.
        // You can add rules by etcdctl in command line.
        let value = r#"[
            {
                "resource": "task",
                "threshold": 1.0
            }
        ]"#;
        client.kv().put(PutRequest::new(key, value)).await?;
    }

    // Sleep 3 seconds and then read the etcd
    sentinel_core::utils::sleep_for_ms(3000);

    // Create a data source and change the rule.
    let h = new_flow_rule_handler(rule_json_array_parser);
    let mut ds = Etcdv3DataSource::new(client, key.into(), vec![h]);
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
