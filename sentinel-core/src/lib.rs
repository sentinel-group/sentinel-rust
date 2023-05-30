#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]

//! # Sentinel in Rust
//!
//! Sentinel takes "flow" as breakthrough point, and works on multiple fields including **flow control**,
//! **traffic shaping**, **circuit breaking** and **system adaptive protection**,
//! to guarantee reliability and resilience for microservices.
//!
//! Sentinel adopts Chain-of-Responsibility pattern. The user-defined rules will be automatically checked via slots in `base::SlotChain`.
//! Generally, there are several steps when using Sentienl:
//! 1. Add dependancy and initialize configurations on Sentinel.
//! 2. Define a resource to be protected and build Sentinel entry.
//! 3. Load the rules defined for each resource.
//! 4. Write the codes at entry and exit points.
//!
//! Thorough examples have been provided in our [repository](https://github.com/sentinel-group/sentinel-rust).
//!
//! ## Add Dependency
//!
//! Add the dependency in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! sentinel-core = { version = "0.1.0", features = ["full"] }
//! ```
//!
//! Optional features lists:
//! - macro：Support procedural macro, simplify the resource and rule definitions, refer to [example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/rules/flow/macro.rs).
//! - async：Support asynchronous resources, refer to [example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/rules/flow/tokio.rs).
//! - exporter：Export metric statistics to Prometheus, refer to [example](https://github.com/sentinel-group/sentinel-rust/tree/main/examples/exporter/prometheus) and [Sentinel Prometheus Metrics Definitions](https://github.com/sentinel-group/sentinel-rust/blob/main/sentinel-core/src/exporter.rs).
//! - logger_env: Use `env_logger` to initialize logging.
//! - logger_log4rs: Use `log4rs` to initialize logging.
//! - ds_consul: Use [Consul](https://www.consul.io/) to configure rules dynamically.
//! - ds_etcdv3：Use [etcd](https://etcd.io/) to configure rules dynamically.
//! - ds_k8s：Use k8s to configure rules dynamically.
//! - metric_log: Store formatted logs on Sentinel resources.
//!
//! ## General Configurations and Initialization
//!
//! Sentinel needs to be initialized. The `api` module provides following interfaces:
//!
//! - `init_default()`: Load configurations from environment variable. For undefined configurations, use default values.
//! - `init_with_config_file(config_path: &mut String)`: Load configurations from a YAML file, refer to [example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/config/from_yaml/from_yaml.rs)。。
//! - `init_with_config(config_entity: ConfigEntity)`: Use hand-crafted `ConfigEntity` to initialize Sentinel，refer to [example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/config/from_entity/from_entity.rs)。。
//!
//!
//! Example:
//!
//! ```rust
//! use sentinel_rs::{init_default, logging};
//! init_default().unwrap_or_else(|err| logging::error!("{:?}", err));
//! ```
//!
//! ## Resouce Definition
//!
//! A snippet of codes is regarded as resources in Sentinel, which can be protected by defining their entries.
//!
//! By constructing `EntryBuilder` and calling the `build()` method, we create `Entry`.
//!
//! Example：
//!
//! If the calling is blocked, `build()` will return an error.
//!
//! ```rust
//! use sentinel_core::base;
//! use sentinel_core::api::EntryBuilder;
//! let entry_builder = EntryBuilder::new(res_name.clone())
//!     .with_traffic_type(base::TrafficType::Inbound);
//! if let Ok(entry) = entry_builder.build() {
//!     // The request is allowed to be processed.
//!     // after finish the logic, exit the entry.
//!     entry.exit()
//! } else {
//!     // The request is blocked.
//!     // you do not need to call `exit()` on entry now.
//! }
//! ```
//!
//! ## Load Sentinel Rules
//!
//! ### Manually Create Sentinel Entry and Load Rules
//!
//! Sentinel supports loading hand-crafted rules.
//! The method `load_rules()` will overload all of the rules defined before.
//! The method `append_rules()` will append rules incrementally.
//! Currently, this is the only way to define several rules for a single resource.
//! For example:
//!  
//! ```rust
//! flow::load_rules(vec![Arc::new(flow::Rule {
//!     resource: "example".into(),
//!     threshold: 10.0,
//!     calculate_strategy: flow::CalculateStrategy::Direct,
//!     control_strategy: flow::ControlStrategy::Reject,
//!     ..Default::default()
//! })]);
//! ```
//!
//! ### Via Attribute-Like Macros
//! We also provide macros to help you define Sentinel resources and load rules easily:
//!
//! ```rust
//! #[flow(threshold=10.0, calculate_strategy=Direct)]
//! pub fn task() -> u32 {}
//! ```
//! When using macro, the resource name will be automatically generated as the method name.
//! Since there is not function overloading in Rust, the resource name will be unique.
//! Sentinel will check if the rule with the same resource name has been loaded.
//!
//! In the example above, the macro modify the function signature of `task`,
//! returning `Result<u32, String>` in the new one.
//! Then, it appends rules to the rule manager, call `EntryBuilder` to create Sentinel entry,
//! check if the entry pass the rule checking. If the tasks is carried successfully,
//! it will return an `Ok(u32)`. Otherwise, `Err(String)` will be returned.
//!
//! The shortcoming is that there is no way to define several rules on a single resource with this macro.
//!
//! ### Via Dynamic Datasource
//!
//! Sentinel supports use dynamically load Sentinel rules, refer to
//!
//! - [etcd example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/datasources/etcdv3.rs)
//! - [consul example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/datasources/consul.rs)。
//! - [k8s example](https://github.com/sentinel-group/sentinel-rust/blob/main/examples/datasources/k8s.rs)。
//!
//! ## More Resources
//!
//! See the [**Wiki**](https://github.com/sentinel-group/sentinel-rust/wiki) for
//! full documentation, examples, blog posts, operational details and other information.
//!
//! See the [Sentinel](https://sentinelguard.io/en-us/) for the document website.
//!
//! See the [中文文档](https://sentinelguard.io/zh-cn/) for document in Chinese.
//!
// This module is not intended to be part of the public API. In general, any
// `doc(hidden)` code is not part of Sentinel's public and stable API.
#[macro_use]
#[doc(hidden)]
pub mod macros;

/// Sentinel API
pub mod api;
/// Core implementations of Sentinel, including the statistic structures,
/// such as the slding window and its underlying LeapArray, the rule managers,
///  and other utilities on configuration and metric logs.
/// The rule managers are responsible for managing the flow controller, circuit breaker,
/// isolation and system status related rules.
pub mod core;
/// Adapters for different logging crates.
pub mod logging;
cfg_exporter! {
    /// Metric Exporter implementations. Currently, only Prometheus is supported.
    pub mod exporter;
}
cfg_datasource! {
    /// Dynamic datasource support for Sentinel rule management.
    /// Currently, k8s, etcd and consul are supported.
    pub mod datasource;
}
// Utility functions for Sentinel.
pub mod utils;

// re-export precludes
pub use crate::core::*;
pub use api::*;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;
