<img src="https://user-images.githubusercontent.com/9434884/43697219-3cb4ef3a-9975-11e8-9a9c-73f4f537442d.png" alt="Sentinel Logo" width="50%">

# Sentinel in Tonic

Implement [Sentinel](https://github.com/sentinel-group/sentinel-rust) in [Tonic](https://github.com/hyperium/tonic). 

The are two kinds of middlewares in tonic.
- [`tonic::service::interceptor::Interceptor`](https://docs.rs/tonic/latest/tonic/service/interceptor/trait.Interceptor.html)
- [`tower::Service`](https://docs.rs/tower/latest/tower/trait.Service.html)

In the `example` directory, we provide an example based on interceptors. 
For the example for services, read the example for `sentinel-tower`.

## Why Setinel?

Though `tonic::transport::channel::Endpoint` does provide methods like `rate_limit()` to construct middleware like `tower::limit::rate::RateLimitLayer()`,
Sentinel provides more general and flexible high-reliability services. 

And Sentinel will be the default traffic governance implentation for the [OpenSergo](https://github.com/opensergo/opensergo-specification) standard.