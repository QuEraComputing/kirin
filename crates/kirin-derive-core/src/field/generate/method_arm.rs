use proc_macro2::{Span, TokenStream};

use super::super::{fields::Fields, info::FieldIterInfo};
use crate::data::*;

pub trait MethodArm {
    fn generate_method_arm(
        &self,
        mutable: bool,
        enum_name: &syn::Ident,
        iter_name: &syn::Ident,
        trait_path: &syn::Path,
        method_name: &syn::Ident,
        item: &TokenStream,
    ) -> TokenStream;
}

impl MethodArm for RegularVariant<'_, FieldIterInfo> {
    fn generate_method_arm(
        &self,
        mutable: bool,
        enum_name: &syn::Ident,
        iter_name: &syn::Ident,
        _trait_path: &syn::Path,
        _method_name: &syn::Ident,
        item: &TokenStream,
    ) -> TokenStream {
        let variant_name = self.variant_name;
        match &self.fields {
            Fields::Named(fields) => {
                let vars = fields.vars();
                let iter = fields.iterator(mutable, &item);
                quote::quote! {
                    #enum_name::#variant_name { #(#vars,)* .. } => {
                        #iter_name::#variant_name ( #iter )
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let vars = fields.vars();
                let iter = fields.iterator(mutable, &item);
                quote::quote! {
                    #enum_name::#variant_name ( #(#vars,)* .. ) => {
                        #iter_name::#variant_name ( #iter )
                    }
                }
            }
            Fields::Unit => {
                quote::quote! {
                    #enum_name::#variant_name => {
                        #iter_name::#variant_name ( std::iter::empty::<#item>() )
                    }
                }
            }
        }
    }
}

impl MethodArm for EitherVariant<'_, FieldIterInfo> {
    fn generate_method_arm(
        &self,
        mutable: bool,
        enum_name: &syn::Ident,
        iter_name: &syn::Ident,
        trait_path: &syn::Path,
        method_name: &syn::Ident,
        item: &TokenStream,
    ) -> TokenStream {
        match self {
            EitherVariant::Regular(variant) => variant.generate_method_arm(
                mutable,
                enum_name,
                iter_name,
                trait_path,
                method_name,
                item,
            ),
            EitherVariant::Wrapper(variant) => variant.generate_method_arm(
                mutable,
                enum_name,
                iter_name,
                trait_path,
                method_name,
                item,
            ),
        }
    }
}

impl MethodArm for WrapperVariant<'_, FieldIterInfo> {
    fn generate_method_arm(
        &self,
        mutable: bool,
        enum_name: &syn::Ident,
        iter_name: &syn::Ident,
        trait_path: &syn::Path,
        method_name: &syn::Ident,
        item: &TokenStream,
    ) -> TokenStream {
        match self {
            WrapperVariant::Named(variant) => variant.generate_method_arm(
                mutable,
                enum_name,
                iter_name,
                trait_path,
                method_name,
                item,
            ),
            WrapperVariant::Unnamed(variant) => variant.generate_method_arm(
                mutable,
                enum_name,
                iter_name,
                trait_path,
                method_name,
                item,
            ),
        }
    }
}

impl MethodArm for NamedWrapperVariant<'_, FieldIterInfo> {
    fn generate_method_arm(
        &self,
        _mutable: bool,
        enum_name: &syn::Ident,
        iter_name: &syn::Ident,
        trait_path: &syn::Path,
        method_name: &syn::Ident,
        _item: &TokenStream,
    ) -> TokenStream {
        let variant_name = self.variant_name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;

        quote::quote! {
            #enum_name::#variant_name { #wraps, .. } => {
                #iter_name::#variant_name ( <#wraps_type as #trait_path>::#method_name(#wraps) )
            },
        }
    }
}

impl MethodArm for UnnamedWrapperVariant<'_, FieldIterInfo> {
    fn generate_method_arm(
        &self,
        _mutable: bool,
        enum_name: &syn::Ident,
        iter_name: &syn::Ident,
        trait_path: &syn::Path,
        method_name: &syn::Ident,
        _item: &TokenStream,
    ) -> TokenStream {
        let variant_name = self.variant_name;
        let wraps_index = self.wraps;
        let wraps_type = &self.wraps_type;
        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps_name = &vars[wraps_index];

        quote::quote! {
            #enum_name::#variant_name (#(#vars,)* ..) => {
                #iter_name::#variant_name ( <#wraps_type as #trait_path>::#method_name(#wraps_name) )
            },
        }
    }
}
