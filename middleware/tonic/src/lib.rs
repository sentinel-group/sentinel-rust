#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]
//! This crate provides the [sentinel](https://docs.rs/sentinel-core) middleware for [tonic](https://docs.rs/tonic).
//! The are two kinds of middlewares in tonic.
//! - [`SentinelInterceptor`] based on [`tonic::service::interceptor::Interceptor`](https://docs.rs/tonic/latest/tonic/service/interceptor/trait.Interceptor.html)
//! - [`SentinelService`] and [`SentinelLayer`] based on [`tower::Service`](https://docs.rs/tower/latest/tower/trait.Service.html)
//! See [examples](https://github.com/sentinel-group/sentinel-rust/tree/main/middleware) for help.

use sentinel_core::EntryBuilder;
pub use sentinel_tower::{SentinelLayer, SentinelService, ServiceRole};
use tonic::service::interceptor::Interceptor;
use tonic::{Request, Status};

pub type Extractor = fn(&Request<()>) -> String;
pub type Fallback = fn(&Request<()>, &sentinel_core::Error) -> Result<Request<()>, Status>;

#[derive(Clone)]
pub struct SentinelInterceptor {
    traffic_type: sentinel_core::base::TrafficType,
    extractor: Option<Extractor>,
    fallback: Option<Fallback>,
}

impl SentinelInterceptor {
    pub fn new(role: ServiceRole) -> Self {
        Self {
            extractor: None,
            fallback: None,
            traffic_type: {
                match role {
                    ServiceRole::Server => sentinel_core::base::TrafficType::Inbound,
                    ServiceRole::Client => sentinel_core::base::TrafficType::Outbound,
                }
            },
        }
    }

    pub fn with_extractor(mut self, extractor: Extractor) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

impl Interceptor for SentinelInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        let resource = match self.extractor {
            Some(extractor) => extractor(&request),
            None => format!("{:?}", request.metadata()),
        };
        let entry_builder = EntryBuilder::new(resource).with_traffic_type(self.traffic_type);
        match entry_builder.build() {
            Ok(entry) => {
                entry.exit();
                Ok(request)
            }
            Err(err) => match self.fallback {
                Some(fallback) => fallback(&request, &err),
                None => Err(Status::resource_exhausted(format!(
                    "Blocked by Sentinel: {:?}",
                    err
                ))),
            },
        }
    }
}
