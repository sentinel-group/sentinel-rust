use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, ItemFn, ReturnType};

// macro_todo: Maybe impl `darling::FromMeta` for Enum is better?
// macro_todo: Maybe refactor the `sentinel-rs` crate, elimnate cyclic dependcies,
// can reduce this redundant definition?
// Refer to crate `rocket_codegen::http_codegen`.
#[derive(Debug, FromMeta)]
pub(crate) struct Rule {
    #[darling(default)]
    pub threshold: Option<f64>,
    #[darling(default)]
    pub traffic_type: Option<String>,
    #[darling(default)]
    pub calculate_strategy: Option<String>,
    #[darling(default)]
    pub control_strategy: Option<String>,
    #[darling(default)]
    pub relation_strategy: Option<String>,
    #[darling(default)]
    pub warm_up_period_sec: Option<u32>,
    #[darling(default)]
    pub warm_up_cold_factor: Option<u32>,
    #[darling(default)]
    pub max_queueing_time_ms: Option<u32>,
    #[darling(default)]
    pub stat_interval_ms: Option<u32>,
    #[darling(default)]
    pub low_mem_usage_threshold: Option<u64>,
    #[darling(default)]
    pub high_mem_usage_threshold: Option<u64>,
    #[darling(default)]
    pub mem_low_water_mark: Option<u64>,
    #[darling(default)]
    pub mem_high_water_mark: Option<u64>,
}

pub(crate) fn build(attr: TokenStream, func: TokenStream) -> TokenStream {
    let attr = parse_macro_input!(attr as AttributeArgs);
    let rule = match Rule::from_list(&attr) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    let func = parse_macro_input!(func as ItemFn);
    let func = process_func(func);
    wrap_sentinel(rule, func)
}

/// Extract the original ReturnType and wrap it with Result<T,E>
fn process_func(mut func: ItemFn) -> ItemFn {
    let return_type = func.sig.output;
    // Currently, use quote/syn to automatically generate it,
    // don't know if there is a better way.
    // Seems hard to parse new ReturnType only or construct ReturnType by hand.
    let dummy_func = match return_type {
        ReturnType::Default => {
            quote! {
                fn dummy() -> Result<(), String> {}
            }
        }
        _ => {
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

/// build the sentinel entry
fn wrap_sentinel(rule: Rule, func: ItemFn) -> TokenStream {
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = func;
    let stmts = &block.stmts;
    let resource_name = sig.ident.to_string();
    let traffic_type = parse_traffic(&rule);
    let rule = process_rule(&resource_name, &rule);
    let expanded = quote! {
        #(#attrs)* #vis #sig {
            use sentinel_rs::{base, flow, EntryBuilder};
            use std::sync::Arc;

            // Load sentinel rules
            flow::load_rules(vec![Arc::new(#rule)]);

            let entry_builder = EntryBuilder::new(String::from(#resource_name))
                .with_traffic_type(#traffic_type);
            match entry_builder.build() {
                Ok(entry) => {
                    // Passed, wrap the logic here.
                    let result = {#(#stmts)*};
                    // Be sure the entry is exited finally.
                    entry.borrow().exit();
                    Ok(result)
                },
                Err(err) => {
                    Err(format!("{:?}", err))
                }
            }
        }
    };
    expanded.into()
}

fn process_rule(resource_name: &String, rule: &Rule) -> TokenStream2 {
    let Rule {
        calculate_strategy,
        control_strategy,
        threshold,
        warm_up_period_sec,
        warm_up_cold_factor,
        max_queueing_time_ms,
        stat_interval_ms,
        low_mem_usage_threshold,
        high_mem_usage_threshold,
        mem_low_water_mark,
        mem_high_water_mark,
        ..
    } = rule;
    let strategy = parse_strategy(calculate_strategy, control_strategy);
    let optional_params = expand_attribute!(
        threshold,
        warm_up_period_sec,
        warm_up_cold_factor,
        max_queueing_time_ms,
        stat_interval_ms,
        low_mem_usage_threshold,
        high_mem_usage_threshold,
        mem_low_water_mark,
        mem_high_water_mark
    );
    quote! {
        flow::Rule {
            id: String::from(#resource_name), // incase of duplication
            resource: String::from(#resource_name),
            ref_resource: String::from(#resource_name),
            relation_strategy: flow::RelationStrategy::CurrentResource,
            #strategy
            #optional_params
            ..Default::default()
        }
    }
}

fn parse_strategy(cal: &Option<String>, ctrl: &Option<String>) -> TokenStream2 {
    let mut strategy = TokenStream2::new();
    if let Some(val) = cal {
        strategy.extend(match &val[..] {
            "Direct" => quote! {calculate_strategy: flow::CalculateStrategy::Direct,},
            "WarmUp" => quote! {calculate_strategy: flow::CalculateStrategy::WarmUp,},
            "MemoryAdaptive" => {
                quote! {calculate_strategy: flow::CalculateStrategy::MemoryAdaptive,}
            }
            _ => quote! {},
        })
    }
    if let Some(val) = ctrl {
        strategy.extend(match &val[..] {
            "Reject" => quote! {control_strategy: flow::ControlStrategy::Reject,},
            "Throttling" => quote! {control_strategy: flow::ControlStrategy::Throttling,},
            _ => quote! {},
        })
    }
    strategy
}

fn parse_traffic(rule: &Rule) -> TokenStream2 {
    let Rule { traffic_type, .. } = rule;
    let mut traffic = TokenStream2::new();
    if let Some(val) = traffic_type {
        traffic.extend(match &val[..] {
            "Outbound" => quote! {base::TrafficType::Outbound},
            _ => quote! {base::TrafficType::Inbound},
        })
    }
    traffic
}
