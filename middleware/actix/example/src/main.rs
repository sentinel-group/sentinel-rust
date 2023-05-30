use actix_web::{
    body::EitherBody,
    dev::{ServiceRequest, ServiceResponse},
    get,
    http::StatusCode,
    App, Error, HttpResponse, HttpServer,
};
use sentinel_actix::Sentinel;
use sentinel_core::flow;
use std::sync::Arc;

const RESOURCE_NAME: &str = "actix_example";

fn custom_extractor(_req: &ServiceRequest) -> String {
    RESOURCE_NAME.into()
}

fn custom_fallback<B>(
    req: ServiceRequest,
    _: &sentinel_core::Error,
) -> Result<ServiceResponse<EitherBody<B>>, Error> {
    Ok(req.into_response(HttpResponse::new(StatusCode::IM_A_TEAPOT).map_into_right_body()))
}

#[get("/")]
async fn index() -> &'static str {
    "Hello world!"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
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

    HttpServer::new(|| {
        App::new()
            .wrap(
                Sentinel::default()
                    .with_extractor(custom_extractor)
                    .with_fallback(custom_fallback),
            )
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
