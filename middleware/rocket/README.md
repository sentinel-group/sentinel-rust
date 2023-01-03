<img src="https://user-images.githubusercontent.com/9434884/43697219-3cb4ef3a-9975-11e8-9a9c-73f4f537442d.png" alt="Sentinel Logo" width="50%">

# Sentinel in Rocket

Implement [Sentinel](https://crates.io/crates/sentinel-core) service in [Rocket](https://crates.io/crates/rocket). 

The Rocket provides two ways to implement middlewares, [request guards](https://rocket.rs/v0.5-rc/guide/requests/#request-guards) and [fairing](https://rocket.rs/v0.5-rc/guide/fairings).

In the `example` directory, we provide an example for both two cases. 

## Why Setinel?

Sentinel provides flexible high-reliability protection for your Rocket services. 

And Sentinel will be the default traffic governance implentation for the [OpenSergo](https://github.com/opensergo/opensergo-specification) standard.