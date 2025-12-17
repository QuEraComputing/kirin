use super::item::MatchingItem;
use crate::{
    data::*,
    kirin::field::{FieldsIter, extra::FieldExtra},
    target,
};
use proc_macro2::TokenStream;
use quote::quote;

target! {
    /// Expression producing the field iterator
    pub struct Expr
}

impl<'src, S> Compile<'src, Statement<'src, S, FieldsIter>, Expr> for FieldsIter {
    fn compile(&self, node: &Statement<'src, S, FieldsIter>) -> Expr {
        let item: MatchingItem = self.compile(node);
        let tokens = node
            .fields
            .iter()
            .filter_map(|f| match f.extra {
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
