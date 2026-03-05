use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// A code fragment that is either a bare expression or a braced block.
///
/// `Expr` renders tokens directly; `Block` wraps them in `{ ... }`.
pub enum Fragment {
    /// A bare expression (rendered as-is).
    Expr(TokenStream),
    /// A block body (wrapped in braces when rendered).
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
