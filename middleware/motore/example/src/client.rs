use sentinel_core::flow;
use sentinel_motore::{BoxError, SentinelLayer, ServiceRole};
use std::net::SocketAddr;
use std::sync::Arc;
use volo_gen::proto_gen::hello::{
    HelloRequest, HelloServiceClientBuilder, HelloServiceRequestSend, HelloServiceResponseRecv,
};
use volo_grpc::{
    context::ClientContext,
    status::{Code, Status},
    Request, Response,
};

const RESOURCE_NAME: &str = "motore_example";

fn custom_extractor(_cx: &ClientContext, _req: &Request<HelloServiceRequestSend>) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback(
    _cx: &ClientContext,
    _req: &Request<HelloServiceRequestSend>,
    err: &sentinel_core::Error,
) -> Result<Response<HelloServiceResponseRecv>, BoxError> {
    Err(Status::new(
        Code::Cancelled,
        format!("Blocked by sentinel at client side: {:?}", err),
    )
    .into())
}

#[volo::main]
async fn main() {
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

    let sentinel = SentinelLayer::new(ServiceRole::Client)
        .with_extractor(custom_extractor)
        .with_fallback(custom_fallback);
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let client = HelloServiceClientBuilder::new("hello")
        .address(addr)
        .layer_outer_front(sentinel)
        .build();

    let mut handlers = Vec::with_capacity(20);
    for _ in 0..10 {
        let mut c = client.clone();
        handlers.push(tokio::spawn(async move {
            let req = HelloRequest {
                name: "Volo".to_string(),
            };
            let resp = c.hello(req).await;
            match resp {
                Ok(info) => println!("{:?}", info),
                Err(e) => eprintln!("{:?}", e),
            }
        }))
    }
    for h in handlers {
        h.await.unwrap();
    }
}
