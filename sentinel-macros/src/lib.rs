//! This crate supplies out-of-the-box attribute macors to ease sentinel usage.  
//! It depends on the [sentinel-rs] crate.
//! [sentinel-rs]:

use proc_macro::TokenStream;

#[macro_use]
#[doc(hidden)]
mod macros;

mod flow;

/// Use this macro by attribute `#[flow()]` to create flow control sentinel.
/// It wraps the task's ReturnType with `Result` to indicate whether the task is blocked
#[proc_macro_attribute]
pub fn flow(attr: TokenStream, item: TokenStream) -> TokenStream {
    flow::build(attr, item)
}
