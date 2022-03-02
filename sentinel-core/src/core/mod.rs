//! Core implementations of Sentinel.

/// Basic definitions, traits, and implementations for Sentinel slot chain, entry, context and so on.
pub mod base;
/// Circuit breaker rules and slots.
pub mod circuitbreaker;
/// Configuration utilities.
pub mod config;
/// Flow control rules and slots.
pub mod flow;
/// Hot key rules and slots.
pub mod hotspot;
/// Resource isolation rules and slots. Currently, only concurrency is supported.
pub mod isolation;
/// Metric logging slots.
pub mod log;
/// Statistic slots and basic data structures,
/// such as the slding window and its underlying LeapArray
pub mod stat;
/// System status rules and slots.
pub mod system;
/// System status collector.
pub mod system_metric;
