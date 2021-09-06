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
