#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]
//! This crate provides the [sentinel](https://docs.rs/sentinel-core) middleware for [actix-web](https://docs.rs/actix-web).
//! See [examples](https://github.com/sentinel-group/sentinel-rust/tree/main/middleware) for help.

use actix_utils::future::{ok, Ready};
use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    Error, HttpResponse,
};
use sentinel_core::EntryBuilder;
use std::{future::Future, pin::Pin, rc::Rc};

/// It is used to extractor a resource name from requests for Sentinel.
pub type Extractor = fn(&ServiceRequest) -> String;

/// The fallback function when service is rejected by sentinel.
pub type Fallback<B> =
    fn(ServiceRequest, &sentinel_core::Error) -> Result<ServiceResponse<EitherBody<B>>, Error>;

/// Sentinel wrapper
pub struct Sentinel<B> {
    extractor: Option<Extractor>,
    fallback: Option<Fallback<B>>,
}

// rustc cannot derive `Default` trait for function pointers correctly,
// implement it by hands
impl<B> Default for Sentinel<B> {
    fn default() -> Self {
        Self {
            extractor: None,
            fallback: None,
        }
    }
}

impl<B> Sentinel<B> {
    pub fn with_extractor(mut self, extractor: Extractor) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<B>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

impl<S, B> Transform<S, ServiceRequest> for Sentinel<B>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = SentinelMiddleware<S, B>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SentinelMiddleware {
            service: Rc::new(service),
            extractor: self.extractor.clone(),
            fallback: self.fallback.clone(),
        })
    }
}

/// Sentinel middleware
pub struct SentinelMiddleware<S, B> {
    service: Rc<S>,
    extractor: Option<Extractor>,
    fallback: Option<Fallback<B>>,
}

impl<S, B> Service<ServiceRequest> for SentinelMiddleware<S, B>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let resource = match self.extractor {
            Some(extractor) => extractor(&req),
            None => req.uri().to_string(),
        };
        let service = Rc::clone(&self.service);
        let entry_builder = EntryBuilder::new(resource)
            .with_traffic_type(sentinel_core::base::TrafficType::Inbound);

        match entry_builder.build() {
            Ok(entry) => Box::pin(async move {
                let response = service
                    .call(req)
                    .await
                    .map(ServiceResponse::map_into_left_body);
                entry.exit();
                response
            }),
            Err(err) => match self.fallback {
                Some(fallback) => Box::pin(async move { fallback(req, &err) }),
                None => Box::pin(async move {
                    Ok(req.into_response(
                        HttpResponse::new(StatusCode::TOO_MANY_REQUESTS).map_into_right_body(),
                    ))
                }),
            },
        }
    }
}
