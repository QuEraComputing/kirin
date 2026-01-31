use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

#[derive(Clone, Debug)]
pub struct FieldPatternTokens {
    named: bool,
    names: Vec<TokenStream>,
}

impl FieldPatternTokens {
    pub fn new(named: bool, names: Vec<TokenStream>) -> Self {
        Self { named, names }
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl ToTokens for FieldPatternTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.names.is_empty() {
            return;
        }
        let names = &self.names;
        if self.named {
            tokens.extend(quote! { { #(#names),* } });
        } else {
            tokens.extend(quote! { ( #(#names),* ) });
        }
    }
}
