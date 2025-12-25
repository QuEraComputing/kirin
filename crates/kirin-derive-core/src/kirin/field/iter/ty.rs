use crate::{
    kirin::field::{context::FieldsIter, extra::FieldExtra, iter::name::Name},
    prelude::*,
};
use proc_macro2::TokenStream;
use quote::quote;

use super::{item::MatchingItem, type_head::TypeGenerics};

target! {
    /// Type of inner iterator for the generated iterator type
    pub struct InnerType
}

impl<'src, T> Compile<'src, FieldsIter, InnerType> for T
where
    T: HasFields<'src, FieldsIter> + WithUserCratePath,
{
    fn compile(&self, ctx: &FieldsIter) -> InnerType {
        let item: MatchingItem = self.compile(ctx);
        let lifetime = &ctx.trait_lifetime;
        let crate_path: CratePath = self.compile(ctx);
        let matching_type = &ctx.matching_type;
        let tokens = self
            .fields()
            .iter()
            .filter_map(|f| match f.extra() {
                FieldExtra::One => Some(quote! { std::iter::Once<#item> }),
                FieldExtra::Vec if ctx.mutable => Some(quote! {
                    std::slice::IterMut<#lifetime, #crate_path :: #matching_type>
                }),
                FieldExtra::Vec => Some(quote! {
                    std::slice::Iter<#lifetime, #crate_path :: #matching_type>
                }),
                FieldExtra::Other => None,
            })
            .fold(None, |acc: Option<TokenStream>, ty| {
                if let Some(acc) = acc {
                    Some(quote! { std::iter::Chain<#acc, #ty>  })
                } else {
                    Some(ty)
                }
            })
            .unwrap_or(quote! { std::iter::Empty<#item> });
        InnerType(tokens)
    }
}

target! {
    /// Full type of the iterator, e.g `MyIter<'a, T>`
    /// can be used within a type expression if the generics
    /// are provided.
    pub struct FullType
}

impl<'src, T> Compile<'src, FieldsIter, FullType> for T
where
    T: Source<Output = &'src syn::DeriveInput>
        + AnyWrapper
        + Compile<'src, FieldsIter, TypeGenerics>
        + Compile<'src, FieldsIter, Name>,
{
    fn compile(&self, ctx: &FieldsIter) -> FullType {
        let name: Name = self.compile(ctx);
        let generics: TypeGenerics = self.compile(ctx);
        let (_, ty_generics, _) = generics.split_for_impl();
        FullType(quote! {
            #name #ty_generics
        })
    }
}
