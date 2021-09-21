use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

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
pub(crate) fn process_rule(resource_name: &str, rule: &Rule) -> TokenStream2 {
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
