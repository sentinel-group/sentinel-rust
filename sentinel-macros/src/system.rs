use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ItemFn;

#[derive(Debug, FromMeta)]
pub(crate) struct Rule {
    #[darling(default)]
    pub threshold: Option<f64>,
    #[darling(default)]
    pub traffic_type: Option<String>,
    #[darling(default)]
    pub metric_type: Option<String>,
    #[darling(default)]
    pub adaptive_strategy: Option<String>,
}

/// build the sentinel entry
pub(crate) fn wrap_sentinel(rule: Rule, func: ItemFn) -> TokenStream {
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
            use sentinel_rs::{base, system, EntryBuilder};
            use std::sync::Arc;
            use sentinel_rs::cfg_if_async;

            // Load sentinel rules
            system::load_rules(vec![Arc::new(#rule)]);

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
}

fn process_rule(resource_name: &String, rule: &Rule) -> TokenStream2 {
    let Rule {
        metric_type,
        threshold,
        adaptive_strategy,
        ..
    } = rule;
    let adaptive_strategy = parse_strategy(adaptive_strategy);
    let metric_type = parse_metric(metric_type);
    let optional_params = expand_attribute!(threshold);
    quote! {
        system::Rule {
            id: String::from(#resource_name), // incase of duplication
            #metric_type
            #adaptive_strategy
            #optional_params
            ..Default::default()
        }
    }
}

fn parse_metric(input: &Option<String>) -> TokenStream2 {
    let mut metric = TokenStream2::new();
    if let Some(val) = input {
        metric.extend(match &val[..] {
            "Load" => quote! {metric_type: system::MetricType::Load,},
            "AvgRT" => quote! {metric_type: system::MetricType::AvgRT,},
            "Concurrency" => quote! {metric_type: system::MetricType::Concurrency,},
            "InboundQPS" => quote! {metric_type: system::MetricType::InboundQPS,},
            "CpuUsage" => quote! {metric_type: system::MetricType::CpuUsage,},
            _ => quote! {},
        })
    }
    metric
}

fn parse_strategy(input: &Option<String>) -> TokenStream2 {
    let mut strategy = TokenStream2::new();
    if let Some(val) = input {
        strategy.extend(match &val[..] {
            "NoAdaptive" => quote! {strategy: system::AdaptiveStrategy::NoAdaptive,},
            "BBR" => quote! {strategy: system::AdaptiveStrategy::BBR,},
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
    } else {
        traffic.extend(quote! {base::TrafficType::Inbound})
    }
    traffic
}
