use proc_macro2::TokenStream;
use quote::quote;

use crate::field::field::NamedMatchingField;

pub struct NamedFields(Vec<NamedMatchingField>);

impl NamedFields {
    pub fn new(fields: &syn::FieldsNamed, matching_type: &syn::Ident) -> syn::Result<Self> {
        Ok(NamedFields(
            fields
                .named
                .iter()
                .map(|f| NamedMatchingField::try_from_field(f, matching_type))
                .collect::<syn::Result<Vec<_>>>()?
                .into_iter()
                .filter_map(|f| f)
                .collect(),
        ))
    }

    pub fn vars(&self) -> Vec<syn::Ident> {
        self.0
            .iter()
            .map(|f| match f {
                NamedMatchingField::One(ident) => ident.clone(),
                NamedMatchingField::Vec(ident) => ident.clone(),
            })
            .collect()
    }

    pub fn iterator(
        &self,
        mutable: bool,
        item: &TokenStream,
    ) -> TokenStream {
        self.0
            .iter()
            .map(|f| match f {
                NamedMatchingField::One(ident) => quote! { std::iter::once(#ident) },
                NamedMatchingField::Vec(ident) => {
                    if mutable {
                        quote! { #ident.iter_mut() }
                    } else {
                        quote! { #ident.iter() }
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
        let mutability = if mutable {
            quote! { mut }
        } else {
            quote! {}
        };

        self.0
            .iter()
            .map(|f| match f {
                NamedMatchingField::One(_) => {
                    quote! { std::iter::Once<&#lifetime #mutability #matching_type_path> }
                }
                NamedMatchingField::Vec(_) => {
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
