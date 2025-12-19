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

impl<'src, T> Compile<'src, T, InnerType> for FieldsIter
where
    T: HasFields<'src, FieldsIter>,
{
    fn compile(&self, node: &T) -> InnerType {
        let item: MatchingItem = self.compile(node);
        let lifetime = &self.trait_lifetime;
        let matching_type = self.absolute_crate_path(&self.matching_type);
        let tokens = node
            .fields()
            .iter()
            .filter_map(|f| match f.extra() {
                FieldExtra::One => Some(quote! { std::iter::Once<#item> }),
                FieldExtra::Vec if self.mutable => Some(quote! {
                    std::slice::IterMut<#lifetime, #matching_type>
                }),
                FieldExtra::Vec => Some(quote! {
                    std::slice::Iter<#lifetime, #matching_type>
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

impl<'src, T> Compile<'src, T, FullType> for FieldsIter
where
    Self: Compile<'src, T, TypeGenerics> + Compile<'src, T, Name>,
    T: Source<Output = &'src syn::DeriveInput> + AnyWrapper,
{
    fn compile(&self, node: &T) -> FullType {
        let name: Name = self.compile(node);
        let generics: TypeGenerics = self.compile(node);
        let (_, ty_generics, _) = generics.split_for_impl();
        FullType(quote! {
            #name #ty_generics
        })
    }
}
