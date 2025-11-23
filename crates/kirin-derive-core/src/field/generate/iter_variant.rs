use proc_macro2::TokenStream;
use quote::quote;

use super::super::{fields::Fields, info::FieldIterInfo};
use crate::data::*;

pub trait IteratorVariant {
    fn generate_iterator_variant(
        &self,
        trait_info: &FieldIterInfo,
        trait_path: &syn::Path,
        trait_ty_generics: &syn::TypeGenerics,
        matching_type: &syn::Path,
        item: &TokenStream,
    ) -> TokenStream;
}

impl IteratorVariant for RegularVariant<'_, FieldIterInfo> {
    fn generate_iterator_variant(
        &self,
        trait_info: &FieldIterInfo,
        _trait_path: &syn::Path,
        _trait_ty_generics: &syn::TypeGenerics,
        matching_type: &syn::Path,
        item: &TokenStream,
    ) -> TokenStream {
        let variant_name = self.variant_name;
        let lifetime = &trait_info.lifetime;
        let iter_type = match &self.fields {
            Fields::Named(fields) => {
                fields.iterator_type(trait_info.mutable, lifetime, matching_type, &item)
            }
            Fields::Unnamed(fields) => {
                fields.iterator_type(trait_info.mutable, lifetime, matching_type, &item)
            }
            Fields::Unit => quote! { std::iter::Empty<#item> },
        };
        quote::quote! {
            #variant_name (#iter_type)
        }
    }
}

impl IteratorVariant for NamedWrapperVariant<'_, FieldIterInfo> {
    fn generate_iterator_variant(
        &self,
        _trait_info: &FieldIterInfo,
        trait_path: &syn::Path,
        trait_ty_generics: &syn::TypeGenerics,
        _matching_type: &syn::Path,
        _item: &TokenStream,
    ) -> TokenStream {
        let variant_name = self.variant_name;
        let wraps_type = &self.wraps_type;
        quote::quote! {
            #variant_name (<#wraps_type as #trait_path #trait_ty_generics>::Iter)
        }
    }
}

impl IteratorVariant for UnnamedWrapperVariant<'_, FieldIterInfo> {
    fn generate_iterator_variant(
        &self,
        _trait_info: &FieldIterInfo,
        trait_path: &syn::Path,
        trait_ty_generics: &syn::TypeGenerics,
        _matching_type: &syn::Path,
        _item: &TokenStream,
    ) -> TokenStream {
        let variant_name = self.variant_name;
        let wraps_type = &self.wraps_type;
        quote::quote! {
            #variant_name (<#wraps_type as #trait_path #trait_ty_generics>::Iter)
        }
    }
}

impl IteratorVariant for WrapperVariant<'_, FieldIterInfo> {
    fn generate_iterator_variant(
        &self,
        trait_info: &FieldIterInfo,
        trait_path: &syn::Path,
        trait_ty_generics: &syn::TypeGenerics,
        matching_type: &syn::Path,
        item: &TokenStream,
    ) -> TokenStream {
        match self {
            WrapperVariant::Named(variant) => {
                variant.generate_iterator_variant(
                    trait_info,
                    trait_path,
                    trait_ty_generics,
                    matching_type,
                    item,
                )
            }
            WrapperVariant::Unnamed(variant) => {
                variant.generate_iterator_variant(
                    trait_info,
                    trait_path,
                    trait_ty_generics,
                    matching_type,
                    item,
                )
            }
        }
    }
}

impl IteratorVariant for EitherVariant<'_, FieldIterInfo> {
    fn generate_iterator_variant(
        &self,
        trait_info: &FieldIterInfo,
        trait_path: &syn::Path,
        trait_ty_generics: &syn::TypeGenerics,
        matching_type: &syn::Path,
        item: &TokenStream,
    ) -> TokenStream {
        match self {
            EitherVariant::Regular(variant) => {
                variant.generate_iterator_variant(
                    trait_info,
                    trait_path,
                    trait_ty_generics,
                    matching_type,
                    item,
                )
            }
            EitherVariant::Wrapper(variant) => {
                variant.generate_iterator_variant(
                    trait_info,
                    trait_path,
                    trait_ty_generics,
                    matching_type,
                    item,
                )
            }
        }
    }
}
