#![allow(unreachable_code)]
use consul::{kv::KVPair, kv::KV, Client, Config, QueryOptions};
use sentinel_core::{
    base,
    datasource::{ds_consul::ConsulDataSource, new_flow_rule_handler, rule_json_array_parser},
    flow,
    utils::sleep_for_ms,
    EntryBuilder, Result,
};
use std::{sync::Arc, thread::JoinHandle};

// An example on consul dynamic data source.
// Install consul, and run `consul agent -data-dir ./` in your terminal first. Then run this example.
// You will find that QPS number is restricted to 10 at first. But soon, it will be restricted to 1.
fn main() -> Result<()> {
    let handlers = basic_flow_example();
    // Create etcd client and put a key-value pair for new rule.
    let config = Config::new().unwrap();
    let client = Client::new(config);
    println!("client: {:?}", client);
    let key = "flow-consul-example";

    {
        // Dynamically add a rule to the consul.
        // You can add rules by etcdctl in command line.

        let value = r#"[{"resource": "task","threshold": 1.0}]"#;
        let pair = KVPair {
            Key: String::from(key),
            Value: String::from(value),
            ..Default::default()
        };

        client.put(&pair, None).unwrap();
        println!("list: {:?}", client.list(key, None));
    }

    // Sleep 3 seconds and then read the consul
    sentinel_core::utils::sleep_for_ms(3000);

    // Create a data source and change the rule.
    let h = new_flow_rule_handler(rule_json_array_parser);
    let mut ds = ConsulDataSource::new(client, QueryOptions::default(), key.into(), vec![h]);
    ds.initialize()?;
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
    Ok(())
}

fn basic_flow_example() -> Vec<JoinHandle<()>> {
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
        handlers.push(std::thread::spawn(move || {
            loop {
                let entry_builder = EntryBuilder::new(res_name.clone())
                    .with_traffic_type(base::TrafficType::Inbound);
                if let Ok(entry) = entry_builder.build() {
                    // Passed, wrap the logic here.
                    task();
                    // Be sure the entry is exited finally.
                    entry.exit()
                } else {
                    sleep_for_ms(100);
                }
            }
        }));
    }
    handlers
}

// todo: Cannot sentinel-macros now. It will append rules,
// which is conflicts with the dynamic datasource
fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(100);
}
