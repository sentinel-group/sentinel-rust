use hello_world::greeter_client::GreeterClient;
use hello_world::HelloRequest;
use sentinel_core::flow;
use sentinel_tower::SentinelService;
use std::sync::Arc;
use tonic::transport::Channel;
use tower::ServiceBuilder;

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

const RESOURCE_NAME: &str = "tonic_example";

type Request = http::Request<tonic::body::BoxBody>;
// type Response = http::Response<tonic::transport::Body>;

fn custom_extractor(_req: &Request) -> String {
    RESOURCE_NAME.into()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::from_static("http://[::1]:50051").connect().await?;

    let channel = ServiceBuilder::new()
        .layer_fn(|s| {
            SentinelService::new(s, sentinel_tower::ServiceRole::Client)
                .with_extractor(custom_extractor)
        })
        .service(channel);

    let client = GreeterClient::new(channel);

    // Init configurations for Sentinel Rust
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    let resource_name = String::from(RESOURCE_NAME);
    // Load sentinel rules
    flow::load_rules(vec![Arc::new(flow::Rule {
        resource: resource_name.clone(),
        threshold: 7.0,
        calculate_strategy: flow::CalculateStrategy::Direct,
        control_strategy: flow::ControlStrategy::Reject,
        ..Default::default()
    })]);

    let mut handlers = Vec::with_capacity(20);
    for _ in 0..10 {
        let mut c = client.clone();
        handlers.push(tokio::spawn(async move {
            let request = tonic::Request::new(HelloRequest {
                name: "Tonic".into(),
            });
            let response = c.say_hello(request).await;
            match response {
                Ok(response) => {
                    println!("RESPONSE={:?}", response.into_inner());
                }
                Err(status) => {
                    println!("Blocked by sentinel: {:?}", status);
                }
            }
        }))
    }
    for h in handlers {
        h.await?;
    }

    Ok(())
}
