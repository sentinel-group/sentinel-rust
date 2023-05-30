use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};
use http::status::StatusCode;
use http_body::Body;
use sentinel_core::flow;
use sentinel_tower::{BoxError, SentinelLayer};
use std::sync::Arc;
use std::time::Duration;
use tonic::{transport::Server, Status};

// seems hard to manipulate the Response in `tower::Service`,
// so we did not implement a custom Fallback example here,
// in fact, it's better to revise the Response,
// instead of propagating errors

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

const RESOURCE_NAME: &str = "tonic_example";

type Request = http::Request<tonic::transport::Body>;
type Response = http::Response<tonic::body::BoxBody>;

fn custom_extractor(_req: &Request) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback(req: &Request, err: sentinel_core::Error) -> Result<Response, BoxError> {
    let resource = req.uri().clone();
    // the message should be encoded with functions in `tonic\src\codec\encode.rs`, but we cannot access them
    let response = http_body::Full::new(format!("Requested resource is: {:?}", resource).into())
        .map_err(move |_| {
            Status::resource_exhausted(format!("Blocked by sentinel at server side: {:?}", err))
        })
        .boxed_unsync();
    Ok(http::response::Builder::new()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(response)
        .unwrap())
}

#[derive(Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: tonic::Request<HelloRequest>,
    ) -> Result<tonic::Response<HelloReply>, tonic::Status> {
        let reply = hello_world::HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };
        Ok(tonic::Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let greeter = MyGreeter::default();

    println!("GreeterServer listening on {}", addr);

    let svc = GreeterServer::new(greeter);

    let sentinel = SentinelLayer::new(sentinel_tower::ServiceRole::Server)
        .with_extractor(custom_extractor)
        .with_fallback(custom_fallback);

    // The stack of middleware that our service will be wrapped in
    let layer = tower::ServiceBuilder::new()
        // Apply middleware from tower
        .timeout(Duration::from_secs(30))
        // Apply our own middleware
        .layer(sentinel)
        .into_inner();

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

    Server::builder()
        // Wrap all services in the middleware stack
        .layer(layer)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
