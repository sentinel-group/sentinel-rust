//! # Sentinel in Rust
//!
//!
#![allow(warnings)]

pub mod api;
pub mod core;
pub mod logging;
pub mod metrics;
pub mod utils;

pub use crate::core::*;
pub use api::*;

use anyhow::{Error, Result};

// todo: replace useless Arc by ref
// returning of getter of Arc should be replaced to ref of Arc, too
// possible use of getter and setter
