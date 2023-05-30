use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};
use sentinel_core::flow;
use sentinel_tonic::SentinelInterceptor;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

const RESOURCE_NAME: &str = "tonic_example";

fn custom_extractor(_req: &Request<()>) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback(_req: &Request<()>, err: &sentinel_core::Error) -> Result<Request<()>, Status> {
    Err(Status::resource_exhausted(format!(
        "Blocked by sentinel at server side: : {:?}",
        err
    )))
}

#[derive(Default, Clone)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        println!("Got a request from {:?}", request.remote_addr());

        let reply = hello_world::HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };
        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let greeter = MyGreeter::default();
    let sentinel = SentinelInterceptor::new(sentinel_tonic::ServiceRole::Server)
        .with_extractor(custom_extractor)
        .with_fallback(custom_fallback);
    let svc = GreeterServer::with_interceptor(greeter, sentinel);

    // Init configurations for Sentinel Rust
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    let resource_name = String::from(RESOURCE_NAME);
    // Load sentinel rules
    flow::load_rules(vec![Arc::new(flow::Rule {
        resource: resource_name.clone(),
        threshold: 4.0,
        calculate_strategy: flow::CalculateStrategy::Direct,
        control_strategy: flow::ControlStrategy::Reject,
        ..Default::default()
    })]);

    println!("GreeterServer listening on {}", addr);
    Server::builder().add_service(svc).serve(addr).await?;
    Ok(())
}
