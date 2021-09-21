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
    pub strategy: Option<String>,
    #[darling(default)]
    pub retry_timeout_ms: Option<u32>,
    #[darling(default)]
    pub min_request_amount: Option<u64>,
    #[darling(default)]
    pub stat_interval_ms: Option<u32>,
    #[darling(default)]
    pub stat_sliding_window_bucket_count: Option<u32>,
    #[darling(default)]
    pub max_allowed_rt_ms: Option<u64>,
}

pub(crate) fn process_rule(resource_name: &String, rule: &Rule) -> TokenStream2 {
    let Rule {
        threshold,
        strategy,
        retry_timeout_ms,
        min_request_amount,
        stat_interval_ms,
        stat_sliding_window_bucket_count,
        max_allowed_rt_ms,
        ..
    } = rule;
    let strategy = parse_strategy(strategy);
    let optional_params = expand_attribute!(
        threshold,
        strategy,
        retry_timeout_ms,
        min_request_amount,
        stat_interval_ms,
        stat_sliding_window_bucket_count,
        max_allowed_rt_ms,
    );
    quote! {
        flow::Rule {
            id: String::from(#resource_name), 
            resource: String::from(#resource_name),
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
            "SlowRequestRatio" => quote! {strategy: circuitbreaker::BreakerStrategy::SlowRequestRatio,},
            "ErrorRatio" => quote! {strategy: circuitbreaker::BreakerStrategy::ErrorRatio,},
            "ErrorCount" => {
                quote! {strategy: circuitbreaker::BreakerStrategy::ErrorCount,}
            }
            _ => quote! {},
        })
    }
    strategy
}
