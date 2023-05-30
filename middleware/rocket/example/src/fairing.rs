use rocket::{catch, catchers, get, http::Status, launch, route, routes, Data, Request};
use sentinel_core::flow;
use sentinel_rocket::{SentinelFairing, SentinelFairingState};
use std::sync::Arc;

const RESOURCE_NAME: &str = "rocket_example";

fn custom_extractor(_req: &Request<'_>) -> String {
    RESOURCE_NAME.into()
}

fn custom_handler<'r>(req: &'r Request<'_>, _data: Data<'r>) -> route::Outcome<'r> {
    match req.rocket().state::<SentinelFairingState>() {
        Some(state) => {
            // by default, the handler will return status 429,
            // but here we simply read the sentinel message and return it to the user with status 200
            let outcome = state.msg.lock().map_or(String::new(), |msg| msg.clone());
            route::Outcome::from(req, outcome)
        }
        None => route::Outcome::Failure(Status::InternalServerError),
    }
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
//   http://127.0.0.1:8000/wave/Rocketeer
#[get("/wave/<name>")]
fn wave(name: &str) -> String {
    format!("👋 Hello, {}!", name)
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

    let sentinel_fairing = SentinelFairing::new("/sentinel")
        .unwrap()
        .with_extractor(custom_extractor)
        .with_handler(custom_handler);

    rocket::build()
        .mount("/", routes![index, wave])
        .register("/", catchers![too_many_request_handler])
        .attach(sentinel_fairing)
}
