# Sentinel in Tower

Implement Sentinel as a service in [Tower](https://github.com/tower-rs/tower). 

In the `example` directory, we provide an example for [tonic](https://github.com/hyperium/tonic). 

## Why Setinel?

Though `tonic::transport::channel::Endpoint` does provide methods like `rate_limit()` to construct middleware like `tower::limit::rate::RateLimitLayer()`,
Sentinel provides more general and flexible high-reliability services. 

And Sentinel will be the default traffic governance implentation for the [OpenSergo](https://github.com/opensergo/opensergo-specification) standard.