macro_rules! expand_attribute {
    ($($attr:expr),*) => {
        {
            let mut token = TokenStream2::new();
            $(if let Some(val) = $attr {
                token.extend(quote!{$attr: #val,});
            })*
            token
        }
    };
}

macro_rules! parse_traffic {
    // fn $name::parse_traffic(rule: $name::Rule) -> base::TrafficType
    ($name:ident,$rule:expr) => {{
        let $name::Rule { traffic_type, .. } = $rule;
        let mut traffic = proc_macro2::TokenStream::new();
        if let Some(val) = traffic_type {
            traffic.extend(match &val[..] {
                "Inbound" => quote::quote! {base::TrafficType::Inbound},
                _ => quote::quote! {base::TrafficType::Inbound},
            })
        } else {
            traffic.extend(quote::quote! {base::TrafficType::Inbound})
        }
        traffic
    }};
}

/// build the sentinel entry
macro_rules! wrap_sentinel {
    // fn $name::wrap_sentinel(rule: $name::Rule, func: ItemFn) -> TokenStream
    ($name:ident,$rule:expr,$func:expr) => {{
        let ItemFn {
            attrs,
            vis,
            sig,
            block,
        } = $func;
        let stmts = &block.stmts;
        let resource_name = sig.ident.to_string();
        let traffic_type = parse_traffic!($name, &$rule);
        let rule = $name::process_rule(&resource_name, &$rule);
        let expanded = quote::quote! {
            #(#attrs)* #vis #sig {
                use sentinel_rs::{base, $name, EntryBuilder};
                use std::sync::Arc;
                use sentinel_rs::cfg_if_async;

                // Load sentinel rules
                $name::load_rules(vec![Arc::new(#rule)]);

                let entry_builder = EntryBuilder::new(String::from(#resource_name))
                    .with_traffic_type(#traffic_type);
                match entry_builder.build() {
                    Ok(entry) => {
                        // Passed, wrap the logic here.
                        let result = {#(#stmts)*};
                        // Be sure the entry is exited finally.
                        cfg_if_async!(entry.read().unwrap().exit(), entry.borrow().exit());
                        Ok(result)
                    },
                    Err(err) => {
                        Err(format!("{:?}", err))
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
            let rule = match $name::Rule::from_list(&attr) {
                Ok(v) => v,
                Err(e) => {
                    return TokenStream::from(e.write_errors());
                }
            };
            let func = parse_macro_input!(func as ItemFn);
            let func = process_func(func);
            wrap_sentinel!($name, rule, func)
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
                fn dummy() -> Result<(), String> {}
            }
        }
        ReturnType::Type(_, return_type) => {
            quote! {
                fn dummy() -> Result<#return_type, String> {}
            }
        }
    };
    let dummy_func: ItemFn = syn::parse2(dummy_func).unwrap();
    // replace the old ReturnType to the dummy function ReturnType
    func.sig.output = dummy_func.sig.output;
    func
}
