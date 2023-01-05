<img src="https://user-images.githubusercontent.com/9434884/43697219-3cb4ef3a-9975-11e8-9a9c-73f4f537442d.png" alt="Sentinel Logo" width="50%">

# Sentinel in Actix

Implement [Sentinel](https://github.com/sentinel-group/sentinel-rust) as a service in [Actix-Web](https://github.com/actix/actix-web). 

In the `example` directory, we provide an example for it. 

## Why Setinel?

Though Actix Web does provide a rate limiter in [actix-limitation](https://crates.io/crates/actix-limitation),
Sentinel provides more general and flexible high-reliability services. 

And Sentinel will be the default traffic governance implentation for the [OpenSergo](https://github.com/opensergo/opensergo-specification) standard.