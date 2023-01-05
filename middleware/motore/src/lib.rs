#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]
#![feature(type_alias_impl_trait)]
//! This crate provides the [Sentinel](https://docs.rs/sentinel-core) service for [Motore](https://github.com/cloudwego/motore).  
//! See [examples](https://github.com/sentinel-group/sentinel-rust/tree/main/middleware) for help.

use core::marker::PhantomData;
use motore::{layer::Layer, service::Service};
use sentinel_core::EntryBuilder;

/// The side where the middleware is deplyed.
#[derive(Debug, Copy, Clone)]
pub enum ServiceRole {
    Server,
    Client,
}

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// It is used to extractor a resource name from requests for sentinel. If you the service request is [`http::Request`](https://docs.rs/http/latest/http/request/struct.Request.html),
/// and you are using nightly toolchain, you don't need to provide a sentinel resource name extractor. The middleware will automatically extract the request uri.
type Extractor<Cx, R> = fn(&Cx, &R) -> String;

/// The fallback function when service is rejected by sentinel.
type Fallback<S, Cx, R> =
    fn(&Cx, &R, &sentinel_core::Error) -> Result<<S as Service<Cx, R>>::Response, BoxError>;

/// The sentinel middleware service in motore.
pub struct SentinelService<S, Cx, R>
where
    S: Service<Cx, R>,
{
    pub(crate) inner: S,
    pub(crate) extractor: Option<Extractor<Cx, R>>,
    pub(crate) fallback: Option<Fallback<S, Cx, R>>,
    traffic_type: sentinel_core::base::TrafficType,
}

// rustc cannot derive `Clone` trait for function pointers correctly,
// implement it by hands
impl<S, Cx, R> Clone for SentinelService<S, Cx, R>
where
    S: Service<Cx, R> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            extractor: self.extractor.clone(),
            fallback: self.fallback.clone(),
            traffic_type: self.traffic_type,
        }
    }
}

impl<S, Cx, R> SentinelService<S, Cx, R>
where
    S: Service<Cx, R> + Clone,
{
    pub fn new(inner: S, role: ServiceRole) -> Self {
        Self {
            inner,
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

    pub fn with_extractor(mut self, extractor: Extractor<Cx, R>) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<S, Cx, R>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

macro_rules! deal_with_sentinel {
    ($resource:ident,$self:ident,$cx:ident,$req:ident) => {{
        let entry_builder = EntryBuilder::new($resource).with_traffic_type($self.traffic_type);
        match entry_builder.build() {
            Ok(entry) => {
                let fut = $self.inner.call($cx, $req).await.map_err(Into::into);
                entry.exit();
                fut
            }
            Err(err) => match $self.fallback {
                Some(fallback) => fallback(&$cx, &$req, &err),
                None => Err(Into::<BoxError>::into(err)),
            },
        }
    }};
}

#[cfg(feature = "volo")]
/// Automatic resource name extraction is supported with [`volo::context::Context`](https://docs.rs/volo/latest/volo/context/trait.Context.html)
#[motore::service]
impl<S, Cx, R> Service<Cx, R> for SentinelService<S, Cx, R>
where
    Cx: 'static + Send + volo::context::Context,
    R: Send + 'static,
    S: Service<Cx, R> + 'static + Send + Clone,
    S::Error: Send + Sync + Into<BoxError>,
{
    async fn call<'cx, 's>(&'s mut self, cx: &'cx mut Cx, req: R) -> Result<S::Response, BoxError>
    where
        's: 'cx,
    {
        let resource = match self.extractor {
            Some(extractor) => extractor(cx, &req),
            None => format!("{:?}", cx.rpc_info()),
        };
        deal_with_sentinel!(resource, self, cx, req)
    }
}

#[cfg(not(feature = "volo"))]
/// For contexts we don't know,
/// we have to provide a [sentinel resource name extractor](Extractor)
#[motore::service]
impl<S, Cx, R> Service<Cx, R> for SentinelService<S, Cx, R>
where
    Cx: 'static + Send,
    R: Send + 'static,
    S: Service<Cx, R> + 'static + Send + Clone,
    S::Error: Send + Sync + Into<BoxError>,
{
    async fn call<'cx, 's>(&'s mut self, cx: &'cx mut Cx, req: R) -> Result<S::Response, BoxError>
    where
        's: 'cx,
    {
        let extractor = self
            .extractor
            .expect("Must provide a resource extractor for unknown Request type");
        let resource = extractor(cx, &req);
        deal_with_sentinel!(resource, self, cx, req)
    }
}

/// The [`motore::Layer`](https://docs.rs/motore/latest/motore/layer/trait.Layer.html) wrapper for [`SentinelService`].
#[derive(Clone)]
pub struct SentinelLayer<S, Cx, R>
where
    S: Service<Cx, R>,
{
    pub(crate) extractor: Option<Extractor<Cx, R>>,
    pub(crate) fallback: Option<Fallback<S, Cx, R>>,
    role: ServiceRole,
    marker: PhantomData<S>,
}

impl<S, Cx, R> Default for SentinelLayer<S, Cx, R>
where
    S: Service<Cx, R> + Clone,
{
    fn default() -> Self {
        Self {
            extractor: None,
            fallback: None,
            role: ServiceRole::Server,
            marker: PhantomData,
        }
    }
}

impl<S, Cx, R> SentinelLayer<S, Cx, R>
where
    S: Service<Cx, R> + Clone,
{
    pub fn new(role: ServiceRole) -> Self {
        Self {
            role,
            ..Default::default()
        }
    }

    pub fn with_extractor(mut self, extractor: Extractor<Cx, R>) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn with_fallback(mut self, fallback: Fallback<S, Cx, R>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

impl<S, Cx, R> Layer<S> for SentinelLayer<S, Cx, R>
where
    S: Service<Cx, R> + Clone,
{
    type Service = SentinelService<S, Cx, R>;

    fn layer(self, inner: S) -> Self::Service {
        let mut s = SentinelService::new(inner, self.role);
        if let Some(e) = self.extractor {
            s = s.with_extractor(e);
        }
        if let Some(f) = self.fallback {
            s = s.with_fallback(f);
        }
        s
    }
}
