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
    pub threshold: Option<u32>,
    #[darling(default)]
    pub metric_type: Option<String>,
}

pub(crate) fn process_rule(resource_name: &str, rule: &Params) -> TokenStream2 {
    let metric_type = parse_metric(&rule.metric_type);
    let optional_params = expand_optional_params!(rule, threshold);
    quote! {
        isolation::Rule {
            id: String::from(#resource_name), // incase of duplication
            resource: String::from(#resource_name),
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
            "Concurrency" => quote! {metric_type: isolation::MetricType::Concurrency,},
            _ => quote! {},
        })
    }
    metric
}
