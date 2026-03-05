use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub enum Fragment {
    Expr(TokenStream),
    Block(TokenStream),
}

impl ToTokens for Fragment {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Fragment::Expr(expr) => tokens.extend(expr.clone()),
            Fragment::Block(block) => tokens.extend(quote! { { #block } }),
        }
    }
}
