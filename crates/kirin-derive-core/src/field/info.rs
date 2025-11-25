use proc_macro2::Span;
use quote::format_ident;

use crate::{
    data::{HasDefaultCratePath, HasGenerics, StatementFields},
    utils::to_camel_case,
};

#[macro_export]
macro_rules! derive_field_iter {
    ($input:expr, $method_name:expr, $matching_type:expr, $trait_path:expr) => {{
        FieldIterInfo::new(
            false,
            $method_name,
            syn::parse_quote! {$matching_type},
            syn::parse_quote! {$trait_path},
        )
        .and_then(|ti| {
            let data = Data::builder().trait_info(&ti).input($input).build();
            Ok(ti.generate_from(&data))
        }).unwrap_or_else(|e| e.to_compile_error())
    }};
}

#[macro_export]
macro_rules! derive_field_iter_mut {
    ($input:expr, $method_name:expr, $matching_type:expr, $trait_path:expr) => {{
        FieldIterInfo::new(
            true,
            $method_name,
            syn::parse_quote! {$matching_type},
            syn::parse_quote! {$trait_path},
        )
        .and_then(|trait_info| {
            let data = Data::builder()
                .trait_info(&trait_info)
                .input($input)
                .build();
            Ok(trait_info.generate_from(&data))
        }).unwrap_or_else(|e| e.to_compile_error())
    }};
}

pub struct FieldIterInfo {
    pub(super) mutable: bool,
    /// method name for the trait being derived
    pub(super) method_name: syn::Ident,
    /// name of the iterator type
    pub(super) iter_name: syn::Ident,
    /// relative path to the trait being derived
    pub(super) trait_path: syn::Path,
    /// relative path to the type being matched
    /// full path is known until we see the crate root
    pub(super) matching_type_path: syn::Path,
    /// name of the type being matched
    pub(super) matching_type_name: syn::Ident,
    /// lifetime for the generated impl
    pub(super) lifetime: syn::Lifetime,
    /// generics of the field iterator trait
    pub(super) generics: syn::Generics,
}

impl FieldIterInfo {
    pub fn new(
        mutable: bool,
        method_name: impl AsRef<str>,
        matching_type: syn::Path,
        trait_path: syn::Path,
    ) -> syn::Result<Self> {
        let method_name_str = method_name.as_ref();
        let iter_name = format_ident!(
            "{}Iter",
            to_camel_case(method_name_str),
            span = Span::call_site()
        );
        let lifetime = syn::Lifetime::new("'a", Span::call_site());
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                lifetime.clone(),
            )));

        let matching_type_name = matching_type
            .segments
            .last()
            .ok_or_else(|| {
                syn::Error::new(
                    Span::call_site(),
                    "Expected matching type to have at least one segment",
                )
            })?
            .ident
            .clone();

        Ok(Self {
            mutable,
            method_name: format_ident!("{}", method_name_str),
            iter_name,
            trait_path,
            matching_type_path: matching_type.clone(),
            matching_type_name,
            lifetime,
            generics,
        })
    }

    pub fn mutability(&self) -> proc_macro2::TokenStream {
        if self.mutable {
            quote::quote! { mut }
        } else {
            quote::quote! {}
        }
    }

    pub fn item(&self, crate_root: &syn::Path) -> proc_macro2::TokenStream {
        let lifetime = &self.lifetime;
        let mut matching_type_path = crate_root.clone();
        matching_type_path
            .segments
            .extend(self.matching_type_path.segments.iter().cloned());
        if self.mutable {
            quote::quote! { & #lifetime mut #matching_type_path }
        } else {
            quote::quote! { & #lifetime #matching_type_path }
        }
    }
}

impl HasDefaultCratePath for FieldIterInfo {
    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::kirin::ir }
    }
}

impl HasGenerics for FieldIterInfo {
    fn generics(&self) -> &syn::Generics {
        &self.generics
    }
}

impl<'a> StatementFields<'a> for FieldIterInfo {
    type FieldsType = super::fields::Fields;
    type InfoType = ();
}
