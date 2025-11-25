use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::field::field::UnnamedMatchingField;

pub struct UnnamedFields(usize, Vec<UnnamedMatchingField>);

impl UnnamedFields {
    pub fn new(fields: &syn::FieldsUnnamed, matching_type: &syn::Ident) -> syn::Result<Self> {
        Ok(UnnamedFields(
            fields.unnamed.len(),
            fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| UnnamedMatchingField::try_from_field(i, f, matching_type))
                .collect::<syn::Result<Vec<_>>>()?
                .into_iter()
                .filter_map(|f| f)
                .collect(),
        ))
    }

    pub fn vars(&self) -> Vec<syn::Ident> {
        (0..self.0).map(|i| format_ident!("field_{}", i)).collect()
    }

    pub fn iterator(&self, mutable: bool, item: &TokenStream) -> TokenStream {
        self.1
            .iter()
            .map(|f| match f {
                UnnamedMatchingField::One(index) => {
                    let var = format_ident!("field_{}", index);
                    quote! { std::iter::once(#var) }
                }
                UnnamedMatchingField::Vec(index) => {
                    let var = format_ident!("field_{}", index);
                    if mutable {
                        quote! { #var.iter_mut() }
                    } else {
                        quote! { #var.iter() }
                    }
                }
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote! { #acc.chain(#field) })
                } else {
                    Some(field)
                }
            })
            .unwrap_or(quote! { std::iter::empty::<#item>() })
    }

    pub fn iterator_type(
        &self,
        mutable: bool,
        lifetime: &syn::Lifetime,
        matching_type_path: &syn::Path,
        item: &TokenStream,
    ) -> TokenStream {
        self.1
            .iter()
            .map(|f| match f {
                UnnamedMatchingField::One(_) => {
                    quote! { std::iter::Once<#item> }
                }
                UnnamedMatchingField::Vec(_) => {
                    if mutable {
                        quote! { std::slice::IterMut<#lifetime, #matching_type_path> }
                    } else {
                        quote! { std::slice::Iter<#lifetime, #matching_type_path> }
                    }
                }
            })
            .fold(None, |acc: Option<TokenStream>, field_type| {
                if let Some(acc) = acc {
                    Some(quote! { std::iter::Chain<#acc, #field_type> })
                } else {
                    Some(field_type)
                }
            })
            .unwrap_or(quote! { std::iter::Empty<#item> })
    }
}
