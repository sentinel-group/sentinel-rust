use hello_world::greeter_client::GreeterClient;
use hello_world::HelloRequest;
use sentinel_core::flow;
use sentinel_tonic::SentinelInterceptor;
use std::sync::Arc;
use tonic::{transport::Channel, Request, Status};

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

const RESOURCE_NAME: &str = "tonic_example";

fn custom_extractor(_req: &Request<()>) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback(_req: &Request<()>, err: &sentinel_core::Error) -> Result<Request<()>, Status> {
    Err(Status::cancelled(format!(
        "Blocked by sentinel at client side: : {:?}",
        err
    )))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        let channel = Channel::from_static("http://[::1]:50051").connect().await?;
        let sentinel = SentinelInterceptor::new(sentinel_tonic::ServiceRole::Client)
            .with_extractor(custom_extractor)
            .with_fallback(custom_fallback);
        let mut c = GreeterClient::with_interceptor(channel, sentinel);
        handlers.push(tokio::spawn(async move {
            let request = tonic::Request::new(HelloRequest {
                name: "Tonic".into(),
            });
            let response = c.say_hello(request).await;
            println!("RESPONSE={:?}", response);
        }))
    }
    for h in handlers {
        h.await?;
    }

    Ok(())
}
