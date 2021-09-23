use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

#[derive(Debug, FromMeta)]
pub(crate) struct Params {
    // sentinel
    #[darling(default)]
    pub traffic_type: Option<String>,
    #[darling(default)]
    pub args: Option<String>,
    // rule
    #[darling(default)]
    pub threshold: Option<u64>,
    #[darling(default)]
    pub metric_type: Option<String>,
    #[darling(default)]
    pub control_strategy: Option<String>,
    #[darling(default)]
    pub param_index: Option<isize>,
    #[darling(default)]
    pub max_queueing_time_ms: Option<u64>,
    #[darling(default)]
    pub burst_count: Option<u64>,
    #[darling(default)]
    pub duration_in_sec: Option<u64>,
    #[darling(default)]
    pub param_max_capacity: Option<usize>,
}

pub(crate) fn process_rule(resource_name: &str, rule: &Params) -> TokenStream2 {
    let control_strategy = parse_strategy(&rule.control_strategy);
    let metric_type = parse_metric(&rule.metric_type);
    let optional_params = expand_optional_params!(
        rule,
        threshold,
        param_index,
        max_queueing_time_ms,
        burst_count,
        duration_in_sec,
        param_max_capacity
    );
    quote! {
        hotspot::Rule {
            id: String::from(#resource_name), // incase of duplication
            resource: String::from(#resource_name),
            #control_strategy
            #metric_type
            #optional_params
            ..Default::default()
        }
    }
}

fn parse_metric(input: &Option<String>) -> TokenStream2 {
    let mut metric = TokenStream2::new();
    if let Some(val) = input {
        metric.extend(match &val[..] {
            "Concurrency" => quote! {metric_type: hotspot::MetricType::Concurrency,},
            "QPS" => quote! {metric_type: hotspot::MetricType::QPS,},
            _ => quote! {},
        })
    }
    metric
}

fn parse_strategy(input: &Option<String>) -> TokenStream2 {
    let mut strategy = TokenStream2::new();
    if let Some(val) = input {
        strategy.extend(match &val[..] {
            "Reject" => quote! {control_strategy: hotspot::ControlStrategy::Reject,},
            "Throttling" => quote! {control_strategy: hotspot::ControlStrategy::Throttling,},
            _ => quote! {},
        })
    }
    strategy
}
