use quote::{ToTokens, quote};

#[derive(Debug, Clone)]
pub struct MatchImpl {
    pub input: syn::Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: syn::Pat,
    pub body: syn::Expr,
}

impl ToTokens for MatchImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let input = &self.input;
        let arms = self.arms.iter().map(|arm| {
            let pattern = &arm.pattern;
            let body = &arm.body;
            quote! {
                #pattern => { #body }
            }
        });

        tokens.extend(quote! {
            match #input {
                #(#arms),*
            }
        });
    }
}

impl MatchImpl {
    pub fn new(input: syn::Expr) -> Self {
        Self {
            input,
            arms: Vec::new(),
        }
    }

    pub fn add_arm(mut self, pattern: syn::Pat, body: syn::Expr) -> Self {
        self.arms.push(MatchArm { pattern, body });
        self
    }
}

impl ToTokens for MatchArm {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let pattern = &self.pattern;
        let body = &self.body;
        tokens.extend(quote! {
            #pattern => { #body }
        });
    }
}
