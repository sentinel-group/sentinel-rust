//! # Sentinel in Rust
//!
//!
#![allow(warnings)]

// This module is not intended to be part of the public API. In general, any
// `doc(hidden)` code is not part of Sentinel's public and stable API.
#[macro_use]
#[doc(hidden)]
pub mod macros;

pub mod api;
pub mod core;
pub mod logging;
cfg_monitor! {
    pub mod monitor;
}
pub mod utils;

pub use crate::core::*;
pub use api::*;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;

// todo: replace useless Arc by ref
// returning of getter of Arc should be replaced to ref of Arc, too
// possible use of getter and setter
