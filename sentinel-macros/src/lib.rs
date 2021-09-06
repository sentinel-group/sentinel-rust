//! This crate supplies out-of-the-box attribute macors to ease sentinel usage.  
//! It depends on the [sentinel-rs] crate.
//! [sentinel-rs]:

use proc_macro::TokenStream;

#[macro_use]
#[doc(hidden)]
mod macros;

mod flow;

/// Use this macro by attribute `#[flow()]`.
/// By default, it simply neglect the blocked task.
#[proc_macro_attribute]
pub fn flow(attr: TokenStream, item: TokenStream) -> TokenStream {
    flow::build(attr, item)
}
