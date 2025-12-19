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

impl<'src, T> Compile<'src, T, Expr> for FieldsIter
where
    T: HasFields<'src, FieldsIter>,
{
    fn compile(&self, node: &T) -> Expr {
        let item: MatchingItem = self.compile(node);
        let tokens = node
            .fields()
            .iter()
            .filter_map(|f| match f.extra() {
                FieldExtra::One => Some(quote! {
                    std::iter::once(#f)
                }),
                FieldExtra::Vec if self.mutable => Some(quote! {
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
