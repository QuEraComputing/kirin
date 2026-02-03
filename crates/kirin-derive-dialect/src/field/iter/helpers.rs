use crate::field::iter::context::DeriveFieldIter;
use kirin_derive_core::prelude::*;
use quote::{format_ident, quote};

pub(crate) struct FieldInputBuilder<'a> {
    pub(crate) ctx: &'a DeriveFieldIter,
    pub(crate) input: &'a InputMeta,
}

impl<'a> FieldInputBuilder<'a> {
    pub(crate) fn new(ctx: &'a DeriveFieldIter, input: &'a InputMeta) -> Self {
        Self { ctx, input }
    }

    pub(crate) fn trait_generics(&self) -> syn::Generics {
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                self.ctx.trait_lifetime.clone(),
            )));
        generics
    }

    pub(crate) fn add_trait_lifetime(&self, generics: &syn::Generics) -> syn::Generics {
        let mut generics = generics.clone();
        let lifetime_ident = &self.ctx.trait_lifetime.ident;
        let has_lifetime = generics
            .lifetimes()
            .any(|lt| lt.lifetime.ident == *lifetime_ident);
        if !has_lifetime {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                    self.ctx.trait_lifetime.clone(),
                )),
            );
        }
        generics
    }

    pub(crate) fn full_trait_path(&self) -> syn::Path {
        let core = self.input.path_builder(&self.ctx.default_crate_path);
        core.full_trait_path(&self.ctx.trait_path)
    }

    pub(crate) fn full_matching_type(&self) -> syn::Path {
        let core = self.input.path_builder(&self.ctx.default_crate_path);
        core.full_path(&self.ctx.matching_type)
    }

    pub(crate) fn iter_type_name(&self) -> syn::Ident {
        format_ident!("{}{}", self.input.name, self.ctx.iter_name)
    }

    pub(crate) fn matching_item(&self) -> proc_macro2::TokenStream {
        let lifetime = &self.ctx.trait_lifetime;
        let matching_type = self.full_matching_type();
        if self.ctx.mutable {
            quote! { &#lifetime mut #matching_type }
        } else {
            quote! { &#lifetime #matching_type }
        }
    }

    pub(crate) fn iter_generics(&self, needs_input_generics: bool) -> syn::Generics {
        if needs_input_generics {
            self.add_trait_lifetime(&self.input.generics)
        } else {
            self.trait_generics()
        }
    }
}

pub(crate) fn field_name_tokens(field: &ir::fields::FieldIndex) -> proc_macro2::TokenStream {
    let name = field.name();
    quote! { #name }
}
