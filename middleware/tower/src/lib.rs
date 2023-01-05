#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]
//! This crate provides the [Sentinel](https://docs.rs/sentinel-core) middleware for [Tower](https://docs.rs/tower).  
//! See the [examples](https://github.com/sentinel-group/sentinel-rust/tree/main/middleware) for help.
//!

use sentinel_core::EntryBuilder;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// It is used to extractor a resource name from requests for Sentinel. If you the service request is [`http::Request`](https://docs.rs/http/latest/http/request/struct.Request.html),
/// and you are using nightly toolchain, you don't need to provide a sentinel resource name extractor. The middleware will automatically extract the request uri.
pub type Extractor<R> = fn(&R) -> String;

/// The fallback function when service is rejected by sentinel.
pub type Fallback<S, R> =
    fn(&R, sentinel_core::Error) -> Result<<S as Service<R>>::Response, BoxError>;

/// The side where the middleware is deplyed.
#[derive(Debug, Copy, Clone)]
pub enum ServiceRole {
    Server,
    Client,
}

/// The sentinel middleware service in tower. If your service request is from [http](https://docs.rs/http) crate,
/// and you are using nightly toolchain, you don't need to provide a [sentinel resource name extractor](Extractor).
/// The middleware will automatically extract the request uri.
pub struct SentinelService<S, R, B = ()>
where
    S: Service<R>,
{
    pub(crate) inner: S,
    pub(crate) extractor: Option<Extractor<R>>,
    pub(crate) fallback: Option<Fallback<S, R>>,
    traffic_type: sentinel_core::base::TrafficType,
    marker: PhantomData<B>,
}

// rustc cannot derive `Clone` trait for function pointers correctly,
// implement it by hands
impl<S, R, B> Clone for SentinelService<S, R, B>
where
    S: Service<R> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: self.marker.clone(),
            extractor: self.extractor.clone(),
            fallback: self.fallback.clone(),
            traffic_type: self.traffic_type,
        }
    }
}

impl<S, R, B> SentinelService<S, R, B>
where
    S: Service<R>,
{
    pub fn new(inner: S, role: ServiceRole) -> Self {
        Self {
            inner,
            marker: PhantomData,
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

    pub fn with_extractor(mut self, extractor: Extractor<R>) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<S, R>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

macro_rules! deal_with_sentinel {
    ($resource:ident,$self:ident,$req:ident) => {{
        let entry_builder = EntryBuilder::new($resource).with_traffic_type($self.traffic_type);
        match entry_builder.build() {
            Ok(entry) => {
                let fut = $self.inner.call($req);
                Box::pin(async move {
                    let response = fut.await.map_err(Into::<BoxError>::into)?;
                    entry.exit();
                    Ok(response)
                })
            }
            Err(err) => match $self.fallback {
                Some(fallback) => {
                    let response = fallback(&$req, err);
                    Box::pin(async move {
                        match response {
                            Ok(response) => Ok(response),
                            Err(err) => Err(err.into()),
                        }
                    })
                }
                None => Box::pin(async move { Err(err.into()) }),
            },
        }
    }};
}

#[cfg(feature = "http")]
/// http` crate is the de-facto standard,
/// widely used in `hyper`, `tonic`, `actix` crates,
/// so here we trait `http` crate as a special case
impl<S, B> Service<http::Request<B>> for SentinelService<S, http::Request<B>, B>
where
    S: Service<http::Request<B>> + Clone + Send + 'static,
    <S as Service<http::Request<B>>>::Future: Send,
    <S as Service<http::Request<B>>>::Response: Send,
    <S as Service<http::Request<B>>>::Error: Into<BoxError>,
    B: Send + 'static,
{
    type Response = <S as Service<http::Request<B>>>::Response;
    type Error = BoxError;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        let resource = match self.extractor {
            Some(extractor) => extractor(&req),
            None => req.uri().to_string(),
        };
        deal_with_sentinel!(resource, self, req)
    }
}

#[cfg(not(feature = "http"))]
/// You have to provide a [sentinel resource name extractor](Extractor)
impl<S, R> Service<R> for SentinelService<S, R>
where
    S: Service<R> + Clone + Send + 'static,
    <S as Service<R>>::Response: Send,
    <S as Service<R>>::Future: Send,
    <S as Service<R>>::Error: Into<BoxError>,
    R: Send + 'static,
{
    type Response = <S as Service<R>>::Response;
    type Error = BoxError;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        let extractor = self
            .extractor
            .expect("Must provide a resource extractor for unknown Request type");
        let resource = extractor(&req);
        deal_with_sentinel!(resource, self, req)
    }
}

/// The [`tower::Layer`](https://docs.rs/tower/latest/tower/trait.Layer.html) wrapper for [`SentinelService`].
pub struct SentinelLayer<S, R, B>
where
    S: Service<R> + Clone,
{
    pub(crate) marker: PhantomData<B>,
    pub(crate) extractor: Option<Extractor<R>>,
    pub(crate) fallback: Option<Fallback<S, R>>,
    pub(crate) role: ServiceRole,
}

// seems rustc cannot derive `Clone` trait for us,
// implement it by hands
impl<S, R, B> Clone for SentinelLayer<S, R, B>
where
    S: Service<R> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            marker: self.marker.clone(),
            extractor: self.extractor.clone(),
            fallback: self.fallback.clone(),
            role: self.role.clone(),
        }
    }
}

impl<S, R, B> Default for SentinelLayer<S, R, B>
where
    S: Service<R> + Clone,
{
    fn default() -> SentinelLayer<S, R, B> {
        Self {
            marker: PhantomData,
            extractor: None,
            fallback: None,
            role: ServiceRole::Server,
        }
    }
}

impl<S, R, B> SentinelLayer<S, R, B>
where
    S: Service<R> + Clone,
{
    pub fn new(role: ServiceRole) -> Self {
        Self {
            role,
            ..Default::default()
        }
    }

    pub fn with_extractor(mut self, extractor: Extractor<R>) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<S, R>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

impl<S, R, B> Layer<S> for SentinelLayer<S, R, B>
where
    S: Service<R> + Clone,
{
    type Service = SentinelService<S, R, B>;

    fn layer(&self, service: S) -> Self::Service {
        let mut s = SentinelService::new(service, self.role);
        if let Some(e) = self.extractor {
            s = s.with_extractor(e);
        }
        if let Some(f) = self.fallback {
            s = s.with_fallback(f);
        }
        s
    }
}
