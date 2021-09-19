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
}

pub(crate) fn process_rule(resource_name: &String, rule: &Rule) -> TokenStream2 {
    let Rule {
        metric_type,
        threshold,
        ..
    } = rule;
    let threshold = threshold.map(|v| v.floor() as u32);
    let metric_type = parse_metric(metric_type);
    let optional_params = expand_attribute!(threshold);
    quote! {
        isolation::Rule {
            id: String::from(#resource_name), // incase of duplication
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
