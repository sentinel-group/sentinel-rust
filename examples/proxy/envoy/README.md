# An Example of Building an Envoy WASM Filter with Sentinel-Rust Flow Control Rules

Based on the marvellous [proxy-wasm crate](https://crates.io/crates/proxy-wasm) and [blog post](https://antweiss.com/blog/extending-envoy-with-wasm-and-rust/).

There is a higher-level [proxy-wasm crate for Envoy](https://github.com/tetratelabs/envoy-wasm-rust-sdk/), but it is based on the experimental version of proxy-wasm crate. And Sentinel aims to provide a general solution for proxies, not only Envoy. Therefore, we choose proxy-wasm crate.

## Building and running:

1. clone this repo
2. `cargo build --target=wasm32-unknown-unknown --release`
3. `docker-compose up --build`

## What this example Filter Does
Each request directed to our service asking a certain service will be counted by Sentinel. If related service calling times exceed the specified thresholds, 5 in this case, Sentinel directly rejects the request and reply a 429 response to the caller.  

Of course, you can play with other Sentinel rules.

## Testing 
We run two server instances in this cluster, a server image from `hashicorp/http-echo` and a mock server with envoy itself. And the load balance strategy of Envoy is set to Round Robin. Therefore, you may get different responses among queries.

Test if the service is on:

```bash
$ curl  -H "user":"Cat" 0.0.0.0:18000
Hi from static service! # Or `Hi from web service!`
```

Test Sentienl:
```bash
$ ./proxy_envoy_test.sh 
```

You will get lots of `Access forbidden.` since QPS (Query/Second) is much larger than the specified threshold 5. 
In this scripts, we visit the cluster 10000 times. You can visit [Envoy Admin Page](http://localhost:18001/stats) to view `*cx_total` statistics on each server.
Increase threshold number, you can get less 429 response.
