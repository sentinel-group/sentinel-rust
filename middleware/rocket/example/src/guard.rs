use rocket::{catch, catchers, get, launch, routes, Request};
use sentinel_core::flow;
use sentinel_rocket::{SentinelConfigForGuard, SentinelGuard};
use std::sync::Arc;

const RESOURCE_NAME: &str = "rocket_example";

fn custom_extractor(_req: &Request<'_>) -> String {
    RESOURCE_NAME.into()
}

#[catch(429)]
fn too_many_request_handler(req: &Request) -> String {
    format!("Request {} too many times!", req.uri())
}

// Try visiting:
//   http://127.0.0.1:8000/
#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

// Try visiting:
//   http://127.0.0.1:8000/guard/Rocketeer
#[get("/guard/<name>")]
fn guard(name: &str, _sg: SentinelGuard) -> String {
    format!("Guard, {}!", name)
}

#[launch]
fn rocket() -> _ {
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

    rocket::build()
        .mount("/", routes![index, guard])
        .register("/", catchers![too_many_request_handler])
        .manage(SentinelConfigForGuard::default().with_extractor(custom_extractor))
}
