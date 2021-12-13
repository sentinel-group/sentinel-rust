use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

// macro_todo: Maybe impl `darling::FromMeta` for Enum is better?
// macro_todo: Maybe refactor the `sentinel-rs` crate, elimnate cyclic dependcies,
// can reduce this redundant definition?
// Refer to crate `rocket_codegen::http_codegen`.
#[derive(Debug, FromMeta)]
pub(crate) struct Params {
    // sentinel
    #[darling(default)]
    pub traffic_type: Option<String>,
    #[darling(default)]
    pub args: Option<String>,
    // rule
    #[darling(default)]
    pub threshold: Option<f64>,
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

pub(crate) fn process_rule(resource_name: &str, rule: &Params) -> TokenStream2 {
    let strategy = parse_strategy(&rule.calculate_strategy, &rule.control_strategy);
    let optional_params = expand_optional_params!(
        rule,
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
    if let Some(val) = ctrl {
        strategy.extend(match &val[..] {
            "Current" => quote! {relation_strategy: flow::RelationStrategy::Current,},
            "Associated" => quote! {relation_strategy: flow::RelationStrategy::Associated,},
            _ => quote! {},
        })
    }
    strategy
}
