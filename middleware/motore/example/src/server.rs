use sentinel_core::flow;
use sentinel_motore::{BoxError, SentinelLayer, ServiceRole};
use std::net::SocketAddr;
use std::sync::Arc;
use volo_gen::proto_gen::hello::{
    HelloRequest, HelloResponse, HelloService, HelloServiceRequestRecv, HelloServiceResponseSend,
    HelloServiceServer,
};
use volo_grpc::{
    context::ServerContext,
    status::{Code, Status},
    Request, Response,
};

const RESOURCE_NAME: &str = "motore_example";

fn custom_extractor(_cx: &ServerContext, _req: &Request<HelloServiceRequestRecv>) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback(
    _cx: &ServerContext,
    _req: &Request<HelloServiceRequestRecv>,
    err: &sentinel_core::Error,
) -> Result<Response<HelloServiceResponseSend>, BoxError> {
    Err(Status::new(
        Code::ResourceExhausted,
        format!("Blocked by sentinel at server side: {:?}", err),
    )
    .into())
}

pub struct S;

#[volo::async_trait]
impl HelloService for S {
    async fn hello(
        &self,
        req: ::volo_grpc::Request<HelloRequest>,
    ) -> Result<::volo_grpc::Response<HelloResponse>, ::volo_grpc::Status> {
        let resp = HelloResponse {
            message: format!("Hello, {}!", req.get_ref().name),
        };
        Ok(::volo_grpc::Response::new(resp))
    }
}

#[volo::main]
async fn main() {
    // Init configurations for Sentinel Rust
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    let resource_name = String::from(RESOURCE_NAME);
    // Load sentinel rules
    flow::load_rules(vec![Arc::new(flow::Rule {
        resource: resource_name.clone(),
        threshold: 3.0,
        calculate_strategy: flow::CalculateStrategy::Direct,
        control_strategy: flow::ControlStrategy::Reject,
        ..Default::default()
    })]);

    let sentinel = SentinelLayer::new(ServiceRole::Server)
        .with_extractor(custom_extractor)
        .with_fallback(custom_fallback);
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let addr = volo::net::Address::from(addr);

    HelloServiceServer::new(S)
        .layer_front(sentinel)
        .run(addr)
        .await
        .unwrap();
}
