macro_rules! expand_optional_params {
    ($params: expr,$($field:ident),*) => {
        {
            let self::Params {
                $($field,)*
                ..
            } = $params;
            let mut token = TokenStream2::new();
            $(if let Some(val) = $field {
                token.extend(quote!{$field: #val,});
            })*
            token
        }
    };
}

#[inline]
pub(crate) fn parse_traffic(input: &Option<String>) -> proc_macro2::TokenStream {
    let mut traffic = proc_macro2::TokenStream::new();
    if let Some(val) = input {
        traffic.extend(match &val[..] {
            "Inbound" => quote::quote! {base::TrafficType::Inbound},
            _ => quote::quote! {base::TrafficType::Inbound},
        })
    } else {
        traffic.extend(quote::quote! {base::TrafficType::Inbound})
    }
    traffic
}

#[inline]
pub(crate) fn parse_args(input: &Option<String>) -> proc_macro2::TokenStream {
    match input {
        Some(input) => {
            let input: proc_macro2::TokenStream = input.parse().unwrap();
            quote::quote! {Some(#input)}
        }
        None => {
            quote::quote! {None}
        }
    }
}

/// build the sentinel entry
macro_rules! wrap_sentinel {
    // fn $name::wrap_sentinel(rule: $name::Rule, func: ItemFn) -> TokenStream
    ($name:ident,$params:expr,$func:expr) => {{
        let ItemFn {
            attrs,
            vis,
            sig,
            block,
        } = $func;
        let stmts = &block.stmts;

        // parse sentinel builder params
        let resource_name = sig.ident.to_string();
        let traffic_type = parse_traffic(&$params.traffic_type);
        let args = parse_args(&$params.args);

        // parse rule params
        let rule = $name::process_rule(&resource_name, &$params);

        // build sentinel entry
        let expanded = quote::quote! {
            #(#attrs)* #vis #sig {
                use sentinel_core::{base, $name, EntryBuilder};
                use std::sync::Arc;

                // Load sentinel rules
                $name::append_rule(Arc::new(#rule));

                let entry_builder = EntryBuilder::new(String::from(#resource_name))
                    .with_traffic_type(#traffic_type)
                    .with_args(#args);
                match entry_builder.build() {
                    Ok(entry) => {
                        // Passed, wrap the logic here.
                        let result = {#(#stmts)*};
                        // Be sure the entry is exited finally.
                        entry.exit();
                        Ok(result)
                    },
                    Err(err) => {
                        Err(err)
                    }
                }
            }
        };
        expanded.into()
    }};
}

macro_rules! build {
    ($name:ident) => {
        /// Use this attribute macro to create the sentinel on your functions/methods.
        /// It wraps the task's ReturnType with `Result` to indicate whether the task is blocked
        #[proc_macro_attribute]
        pub fn $name(attr: TokenStream, func: TokenStream) -> TokenStream {
            use darling::FromMeta;
            use syn::ItemFn;

            let attr = parse_macro_input!(attr as AttributeArgs);
            let params = match $name::Params::from_list(&attr) {
                Ok(v) => v,
                Err(e) => {
                    return TokenStream::from(e.write_errors());
                }
            };
            let func = parse_macro_input!(func as ItemFn);
            let func = process_func(func);
            wrap_sentinel!($name, params, func)
        }
    };
}

use quote::quote;
use syn::{ItemFn, ReturnType};

/// Extract the original ReturnType and wrap it with Result<T,E>
pub(crate) fn process_func(mut func: ItemFn) -> ItemFn {
    let output = func.sig.output;
    // Currently, use quote/syn to automatically generate it,
    // don't know if there is a better way.
    // Seems hard to parse new ReturnType only or construct ReturnType by hand.
    let dummy_func = match output {
        ReturnType::Default => {
            quote! {
                fn dummy() -> sentinel_core::Result<()> {}
            }
        }
        ReturnType::Type(_, return_type) => {
            quote! {
                fn dummy() -> sentinel_core::Result<#return_type> {}
            }
        }
    };
    let dummy_func: ItemFn = syn::parse2(dummy_func).unwrap();
    // replace the old ReturnType to the dummy function ReturnType
    func.sig.output = dummy_func.sig.output;
    func
}
