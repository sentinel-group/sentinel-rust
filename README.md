<img src="https://user-images.githubusercontent.com/9434884/43697219-3cb4ef3a-9975-11e8-9a9c-73f4f537442d.png" alt="Sentinel Logo" width="50%">

# Sentinel: The Sentinel of Your Microservices

[![Crates.io][crates-badge]][crates-url]
[![Sentinel CI][ci-badge]][ci-url]
[![Codecov][codecov-badge]][codecov-url]
[![Apache licensed][apache-badge]][apache-url]
[![Gitter chat][gitter-badge]][gitter-url]


[crates-badge]: https://img.shields.io/crates/v/sentinel-core.svg
[crates-url]: https://crates.io/crates/sentinel-core
[ci-badge]: https://github.com/sentinel-group/sentinel-rust/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/sentinel-group/sentinel-rust/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/gh/sentinel-group/sentinel-rust/branch/main/graph/badge.svg
[codecov-url]: https://codecov.io/gh/sentinel-group/sentinel-rust
[apache-badge]: https://img.shields.io/badge/license-Apache%202-4EB1BA.svg
[apache-url]: https://www.apache.org/licenses/LICENSE-2.0.html
[gitter-badge]: https://badges.gitter.im/alibaba/Sentinel.svg
[gitter-url]: https://gitter.im/alibaba/Sentinel

## Introduction

As distributed systems become increasingly popular, the reliability between services is becoming more important than ever before.
Sentinel takes "flow" as breakthrough point, and works on multiple fields including **flow control**,
**traffic shaping**, **circuit breaking** and **system adaptive protection**, to guarantee reliability and resilience for microservices.

Sentinel has the following features:

- **Rich applicable scenarios**: Sentinel has been wildly used in Alibaba, and has covered almost all the core-scenarios in Double-11 (11.11) Shopping Festivals in the past 10 years, such as “Second Kill” which needs to limit burst flow traffic to meet the system capacity, message peak clipping and valley fills, circuit breaking for unreliable downstream services, cluster flow control, etc.
- **Real-time monitoring**: Sentinel also provides real-time monitoring ability. You can see the runtime information of a single machine in real-time, and pump the metrics to outside metric components like Prometheus.
- **Polyglot support**: Sentinel has provided native support for [Rust](https://github.com/sentinel-group/sentinel-rust), [Java](https://github.com/alibaba/Sentinel), [Go](https://github.com/alibaba/sentinel-golang) and [C++](https://github.com/alibaba/sentinel-cpp).


## Documentation

See the [**Wiki**](https://github.com/sentinel-group/sentinel-rust/wiki) for **Rust version** full documentation, examples, blog posts, operational details and other information.

See the [Sentinel](https://sentinelguard.io/en-us/) for the document website.

See the [中文文档](https://sentinelguard.io/zh-cn/) for document in Chinese.

The [Rust API documentation](https://docs.rs/sentinel-core/latest) is working in progress.

## Example

Add the dependency in Cargo.toml:

```toml
[dependencies]
sentinel-core = { version = "0.1.0", features = ["full"] }
```

## Contributing

Contributions are always welcomed! Please refer to [CONTRIBUTING](./CONTRIBUTING.md) for detailed guidelines.
