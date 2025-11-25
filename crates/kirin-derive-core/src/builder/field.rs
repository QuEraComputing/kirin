use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::data::*;

pub struct FieldInfo {
    pub attr: Option<FieldAttribute>,
    pub name: syn::Ident,
    pub ty: syn::Type,
    pub is_input_ssa: bool,
    pub is_result: bool,
    pub default: Option<syn::Expr>,
}

impl FieldInfo {
    pub fn input_signature(&self, crate_path: &syn::Path) -> TokenStream {
        let FieldInfo {
            name,
            ty,
            is_input_ssa,
            ..
        } = self;
        if *is_input_ssa {
            return quote! { #name: impl Into<#crate_path::SSAValue> };
        }

        match &self.attr {
            Some(FieldAttribute {
                builder: Some(FieldBuilder { into: true, .. }),
                ..
            }) => {
                quote! { #name: impl Into<#ty> }
            }
            Some(FieldAttribute {
                builder:
                    Some(FieldBuilder {
                        default: Some(_), ..
                    }),
                ..
            }) => {
                quote! {}
            }
            _ => quote! { #name: #ty },
        }
    }

    pub fn build_input(&self, crate_path: &syn::Path) -> TokenStream {
        let FieldInfo { name, is_input_ssa, .. } = self;
        if *is_input_ssa {
            return quote! { let #name: #crate_path::SSAValue = #name.into(); };
        }

        match &self.attr {
            Some(FieldAttribute {
                builder: Some(FieldBuilder { into: true, .. }),
                ..
            }) => {
                quote! { let #name = #name.into(); }
            }
            Some(FieldAttribute {
                builder:
                    Some(FieldBuilder {
                        default: Some(init),
                        ..
                    }),
                ..
            }) => {
                quote! { let #name = #init; }
            }
            _ => quote! {},
        }
    }

    /// generate the builder code for ResultValue field
    /// - `statement_id` the statement id variable name
    /// - `index` the index of the result field in the result list
    pub fn build_result(
        &self,
        crate_path: &syn::Path,
        statement_id: &syn::Ident,
        index: usize,
    ) -> TokenStream {
        let name = &self.name;
        let msg = "expect #[kirin(type = ...)] attribute for ResultValue field";
        let ty = self
            .attr
            .as_ref()
            .expect(msg)
            .builder
            .as_ref()
            .expect(msg)
            .ty
            .as_ref();

        if ty.is_none() {
            return syn::Error::new_spanned(&self.ty, msg).to_compile_error();
        }

        quote! {
            let #name: #crate_path::ResultValue = arena
                .ssa()
                .kind(SSAKind::Result(#statement_id, #index))
                .ty(Lang::TypeLattice::from(#ty))
                .new()
                .into();
        }
    }
}

impl std::fmt::Debug for FieldInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldInfo")
            .field("attr", &self.attr)
            .field("name", &self.name)
            .field("ty", &self.ty.to_token_stream())
            .field("is_result", &self.is_result)
            .finish()
    }
}
