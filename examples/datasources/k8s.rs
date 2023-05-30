#![allow(unreachable_code)]
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    runtime::wait::{await_condition, conditions},
    Client, CustomResourceExt,
};
use sentinel_core::{
    base,
    datasource::{
        ds_k8s::{K8sDataSource, SENTINEL_RULE_GROUP},
        new_flow_rule_handler, rule_json_array_parser,
    },
    flow, EntryBuilder, Result,
};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

// An example on k8s dynamic data source.
// Install minikube in your terminal first. Run `minikube start`, then run this example.
// Run `kubectl get flowresources -A` will show the flow resources created by this example.
// Run `kubectl delete flowresources/flow-1` will delete the flow resource created by this example.
// You will find that QPS number is restricted to 10 at first. But soon, it will be restricted to 1.
#[tokio::main]
async fn main() -> Result<()> {
    let handlers = basic_flow_example().await;

    // Create etcd client and put a key-value pair for new rule.
    let client = Client::try_default().await?;
    let property = "flow-k8s-example";
    let namespace = "default";
    let cr_name = "flowresources";

    println!(
        "FlowRule CRD is: \n{}",
        serde_json::to_string_pretty(&flow::FlowResource::crd()).unwrap()
    );

    {
        // Dynamically add a CRD sentinel rule.
        // You can add rules by kubectl in command line.
        dynamic_update(&client, property, namespace, cr_name).await?;
    }

    // Sleep 3 seconds and then read the change of CRD
    sentinel_core::utils::sleep_for_ms(3000);

    // Create a data source and change the rule.
    let h = new_flow_rule_handler(rule_json_array_parser);
    let mut ds = K8sDataSource::<flow::Rule, _, flow::FlowResource>::new(
        client,
        property.into(),
        namespace.into(),
        cr_name.into(),
        vec![h],
    );
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

async fn dynamic_update(
    client: &Client,
    manager: &str,
    namespace: &str,
    cr_name: &str,
) -> Result<()> {
    let crds: Api<CustomResourceDefinition> = Api::all(client.clone());
    println!("Before patch");
    let cr_name = format!("{}.{}", cr_name, SENTINEL_RULE_GROUP);
    // Apply the CRD so users can create Foo instances in Kubernetes
    crds.patch(
        &cr_name,
        &PatchParams::apply(manager),
        &Patch::Apply(flow::FlowResource::crd()),
    )
    .await?;
    println!("After patch");
    // Wait for the CRD to be ready
    await_condition(crds, &cr_name, conditions::is_crd_established()).await?;

    let cr = flow::FlowResource::new(
        "flow-1",
        flow::Rule {
            resource: "task".into(),
            threshold: 1.0,
            ..Default::default()
        },
    );

    let flow_rule: Api<flow::FlowResource> = Api::namespaced(client.clone(), namespace);
    flow_rule.create(&PostParams::default(), &cr).await?;
    println!(
        "Dynamically change custom resource: {} to: {:?}",
        cr_name, cr
    );
    Ok(())
}
