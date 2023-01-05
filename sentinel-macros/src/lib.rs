#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/43955412")]
//! This crate supplies out-of-the-box attribute macors to ease sentinel usage.  
//! It depends on the [sentinel-core] crate.
//! Currently, only one sentinel attribute macro is permited to added on a single function.

#![allow(clippy::needless_update)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, AttributeArgs};

#[macro_use]
#[doc(hidden)]
mod utils;
use utils::*;

mod circuitbreaker;
mod flow;
mod hotspot;
mod isolation;
mod system;

build!(flow);
build!(system);
build!(isolation);
build!(circuitbreaker);
build!(hotspot);
