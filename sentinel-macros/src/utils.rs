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
            $name::wrap_sentinel(rule, func)
        }
    };
}

use quote::quote;
use syn::{ItemFn, ReturnType};

/// Extract the original ReturnType and wrap it with Result<T,E>
pub(crate) fn process_func(mut func: ItemFn) -> ItemFn {
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
