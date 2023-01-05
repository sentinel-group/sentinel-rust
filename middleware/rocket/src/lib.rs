#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]
//! This crate provides the [sentinel](https://docs.rs/sentinel-core) middleware for [actix-web](https://docs.rs/actix-web).
//! See [examples](https://github.com/sentinel-group/sentinel-rust/tree/main/middleware) for help.
//!

use rocket::{
    fairing::{self, Fairing, Info, Kind},
    http::{self, uri::Origin, Method, Status},
    request::{self, FromRequest},
    route, Build, Data, Request, Rocket, Route,
};
use sentinel_core::EntryBuilder;
use std::sync::Mutex;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// It is used to extractor a resource name from requests for Sentinel.
pub type Extractor = fn(&Request<'_>) -> String;

/// The fallback function when service is rejected by sentinel.
pub type Fallback<R> = fn(&Request<'_>, sentinel_core::Error) -> R;

fn default_extractor(req: &Request<'_>) -> String {
    req.uri().path().to_string()
}

fn default_fallback_for_guard(
    _request: &Request<'_>,
    err: sentinel_core::Error,
) -> request::Outcome<SentinelGuard, BoxError> {
    request::Outcome::Failure((Status::TooManyRequests, err.into()))
}

pub type SentinelConfigForGuard = SentinelConfig<request::Outcome<SentinelGuard, BoxError>>;
pub type SentinelConfigForFairing = SentinelConfig<()>;

/// When using [`SentinelGuard`](SentinelGuard), we can only use [managed state][managed_state]
/// to configure the [`SentinelGuard`](SentinelGuard). That is
/// ```rust
/// rocket::build().manage(SentinelConfig { ... });
/// ```
/// For [SentinelFairing](SentinelFairing), the configuration in [managed state][managed_state] is
/// with lower priority than the `SentinelConfig` in it.
///
/// [managed_state]: https://rocket.rs/v0.5-rc/guide/state/#managed-state
pub struct SentinelConfig<R> {
    pub extractor: Option<Extractor>,
    pub fallback: Option<Fallback<R>>,
}

impl<R> SentinelConfig<R> {
    pub fn with_extractor(mut self, extractor: Extractor) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<R>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

// rustc cannot derive `Clone` trait for function pointers correctly,
// implement it by hands
impl<R> Clone for SentinelConfig<R> {
    fn clone(&self) -> Self {
        Self {
            extractor: self.extractor.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

// rustc cannot derive `Default` trait for function pointers correctly,
// implement it by hands
impl<R> Default for SentinelConfig<R> {
    fn default() -> Self {
        Self {
            extractor: None,
            fallback: None,
        }
    }
}

/// The Rocket request guard, which is the recommended way in [Rocket documentation](https://rocket.rs/v0.5-rc/guide/requests/#request-guards).
/// To use this guard, simply add it to the arguments of handler. By default, it extracts [the path in the Request::uri()](https://docs.rs/rocket/0.5.0-rc.2/rocket/struct.Request.html#method.uri) as the sentinel resource name.
/// The blocked requests returns the [status 429](https://docs.rs/rocket/0.5.0-rc.2/rocket/http/struct.Status.html#associatedconstant.TooManyRequests),
/// ```rust
/// #[get("/use_sentinel")]
/// fn use_sentinel(_sentinel: SentinelGuard) { /* .. */ }
/// ```
/// We can use [`SentinelConfig`](SentinelConfig) to configure the guard.
#[derive(Debug)]
pub struct SentinelGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SentinelGuard {
    type Error = BoxError;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let empty_config = SentinelConfig::default();
        let config = req
            .rocket()
            // The type `R` in `SentinelConfig<R>` here is the same as `default_fallback_for_guard`
            .state::<SentinelConfig<request::Outcome<SentinelGuard, BoxError>>>()
            .unwrap_or(&empty_config);
        let extractor = config.extractor.unwrap_or(default_extractor);
        let fallback = config.fallback.unwrap_or(default_fallback_for_guard);

        let resource = extractor(req);
        let entry_builder = EntryBuilder::new(resource)
            .with_traffic_type(sentinel_core::base::TrafficType::Inbound);

        match entry_builder.build() {
            Ok(entry) => {
                entry.exit();
                request::Outcome::Success(SentinelGuard {})
            }
            Err(err) => fallback(req, err),
        }
    }
}

/// The [managed state][managed_state] to be processed in the handler mounted in [SentinelFairing](SentinelFairing).
///
/// [managed_state]: https://rocket.rs/v0.5-rc/guide/state/#managed-state
#[derive(Debug)]
pub struct SentinelFairingState {
    pub msg: Mutex<String>,
    /// the forwarded uri has to be managed by the rocket,
    /// because currently in `Fairing::on_request(&self, req: &mut Request<'_>, _: &mut Data<'_>)`,
    /// the lifetime of `&'life1 self` and `Request<'life2>` are not constrained.
    /// see the source code of [Fairing](https://docs.rs/rocket/0.5.0-rc.2/rocket/fairing/trait.Fairing.html) for details.
    pub uri: String,
}

impl SentinelFairingState {
    pub fn new(uri: String) -> Self {
        Self {
            msg: Mutex::new(String::new()),
            uri,
        }
    }
}

type FairingHandler = for<'r> fn(&'r Request<'_>, Data<'r>) -> route::Outcome<'r>;

#[derive(Clone, Default)]
pub struct SentinelFairingHandler(Option<FairingHandler>);

impl SentinelFairingHandler {
    pub fn new(h: FairingHandler) -> Self {
        Self(Some(h))
    }
}

#[rocket::async_trait]
impl route::Handler for SentinelFairingHandler {
    async fn handle<'r>(&self, req: &'r Request<'_>, data: Data<'r>) -> route::Outcome<'r> {
        fn default_handler<'r>(req: &'r Request<'_>, _data: Data<'r>) -> route::Outcome<'r> {
            match req.rocket().state::<SentinelFairingState>() {
                Some(_) => route::Outcome::Failure(Status::TooManyRequests),
                None => route::Outcome::Failure(Status::InternalServerError),
            }
        }

        let h = self.0.unwrap_or(default_handler);
        h(req, data)
    }
}

impl Into<Vec<Route>> for SentinelFairingHandler {
    fn into(self) -> Vec<Route> {
        vec![Route::new(Method::Get, "/", self)]
    }
}

/// The Rocket Fairing. The [SentinelConfig](SentinelConfig) in
/// SentinelFairing is with higher priority than the one in global [managed state][managed_state].
///
/// [managed_state]: https://rocket.rs/v0.5-rc/guide/state/#managed-state
#[derive(Default)]
pub struct SentinelFairing {
    /// the forwarded page when blocked by sentinel,
    /// which will be handled by the `handler`
    uri: String,
    /// a lightweight handler, which handles all the requests blocked by Sentinel.
    handler: SentinelFairingHandler,
    /// config for `SentinelFairing` itself, which is with higher priority than the `SentinelConfig` in global [managed state][managed_state].
    ///
    /// [managed_state]: https://rocket.rs/v0.5-rc/guide/state/#managed-state
    config: SentinelConfig<()>,
}

impl SentinelFairing {
    pub fn new(uri: &'static str) -> Result<Self, http::uri::Error> {
        Ok(SentinelFairing::default().with_uri(uri)?)
    }

    pub fn with_extractor(mut self, extractor: Extractor) -> Self {
        self.config = self.config.with_extractor(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<()>) -> Self {
        self.config = self.config.with_fallback(fallback);
        self
    }

    pub fn with_handler(mut self, h: FairingHandler) -> Self {
        self.handler = SentinelFairingHandler::new(h);
        self
    }

    pub fn with_uri(mut self, uri: &'static str) -> Result<Self, http::uri::Error> {
        let origin = Origin::parse(uri)?;
        self.uri = origin.path().to_string();
        Ok(self)
    }
}

#[rocket::async_trait]
impl Fairing for SentinelFairing {
    fn info(&self) -> Info {
        Info {
            name: "Sentinel Fairing",
            kind: Kind::Ignite | Kind::Request,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<Build>) -> fairing::Result {
        let handler = self.handler.clone();
        Ok(rocket
            .manage(SentinelFairingState::new(self.uri.clone()))
            .mount(self.uri.clone(), handler))
    }

    async fn on_request(&self, req: &mut Request<'_>, _: &mut Data<'_>) {
        let empty_config = SentinelConfig::default();
        let config = req
            .rocket()
            .state::<SentinelConfig<()>>()
            .unwrap_or(&empty_config);
        let extractor = self
            .config
            .extractor
            .unwrap_or(config.extractor.unwrap_or(default_extractor));
        let fallback = self.config.fallback.or(config.fallback);

        let resource = extractor(&req);
        let entry_builder = EntryBuilder::new(resource)
            .with_traffic_type(sentinel_core::base::TrafficType::Inbound);

        match entry_builder.build() {
            Ok(entry) => {
                entry.exit();
            }
            Err(err) => {
                match fallback {
                    Some(fallback) => fallback(req, err),
                    None => {
                        if let Some(state) = req.rocket().state::<SentinelFairingState>() {
                            if let Ok(mut msg) = state.msg.lock() {
                                *msg = format!(
                                    "Request to {:?} blocked by sentinel: {:?}",
                                    req.uri().path(),
                                    err
                                );
                            }
                            // this `unwrap` call will never fail
                            req.set_uri(Origin::parse(&state.uri).unwrap());
                        }
                    }
                }
            }
        };
    }
}
