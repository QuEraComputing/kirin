use super::item::MatchingItem;
use crate::{
    kirin::field::{context::FieldsIter, extra::FieldExtra},
    prelude::*,
};
use proc_macro2::TokenStream;
use quote::quote;

target! {
    /// Expression producing the field iterator
    pub struct Expr
}

impl<'src, T> Compile<'src, FieldsIter, Expr> for T
where
    T: WithUserCratePath + HasFields<'src, FieldsIter>,
{
    fn compile(&self, ctx: &FieldsIter) -> Expr {
        let item: MatchingItem = self.compile(ctx);
        let tokens = self
            .fields()
            .iter()
            .filter_map(|f| match f.extra() {
                FieldExtra::One => Some(quote! {
                    std::iter::once(#f)
                }),
                FieldExtra::Vec if ctx.mutable => Some(quote! {
                    #f.iter_mut()
                }),
                FieldExtra::Vec => Some(quote! {
                    #f.iter()
                }),
                FieldExtra::Other => None,
            })
            .fold(None, |acc: Option<TokenStream>, iter| {
                if let Some(acc) = acc {
                    Some(quote! { #acc.chain(#iter) })
                } else {
                    Some(iter)
                }
            })
            .unwrap_or(quote! { std::iter::empty::<#item>() });
        Expr(tokens)
    }
}
