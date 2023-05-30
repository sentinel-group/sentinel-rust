use axum::{
    body::{Body, Bytes, Full, HttpBody},
    error_handling::HandleErrorLayer,
    http,
    http::StatusCode,
    response::Response,
    routing::get,
    BoxError, Error, Router,
};
use sentinel_core::flow;
use sentinel_tower::SentinelLayer;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;

const RESOURCE_NAME: &str = "axum_example";

type Request = http::Request<Body>;

fn custom_extractor(_req: &Request) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback(req: &Request, err: sentinel_core::Error) -> Result<Response, BoxError> {
    let resource = req.uri().clone();
    let response = Full::<Bytes>::new(format!("Requested resource is: {:?}.\nHowever, it is blocked by Sentinel, the reported error is:\n{}", resource, err).into())
        .map_err(move |e| Error::new(e))
        .boxed_unsync();
    Ok(http::response::Builder::new()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(response)
        .unwrap())
}

async fn index() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() {
    let sentinel_layer = SentinelLayer::new(sentinel_tower::ServiceRole::Server)
        .with_extractor(custom_extractor)
        .with_fallback(custom_fallback);

    let app = Router::new().route("/", get(index)).layer(
        ServiceBuilder::new()
            // this middleware goes above `SentinelLayer` because it will receive
            // errors returned by `SentinelLayer`
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                StatusCode::INTERNAL_SERVER_ERROR
            }))
            .layer(sentinel_layer),
    );

    // Init configurations for Sentinel Rust
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    let resource_name = String::from(RESOURCE_NAME);
    // Load sentinel rules
    flow::load_rules(vec![Arc::new(flow::Rule {
        resource: resource_name.clone(),
        threshold: 1.0,
        calculate_strategy: flow::CalculateStrategy::Direct,
        control_strategy: flow::ControlStrategy::Reject,
        ..Default::default()
    })]);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
