use std::ops::Deref;

use quote::{ToTokens, quote};

use super::name::Name;
use crate::{prelude::*, kirin::field::context::FieldsIter};

/// Type generics for the generated iterator
pub struct TypeGenerics(syn::Generics);

impl ToTokens for TypeGenerics {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}
impl Deref for TypeGenerics {
    type Target = syn::Generics;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'src> Compile<'src, FieldsIter, TypeGenerics> for Struct<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> TypeGenerics {
        if self.is_wrapper() {
            TypeGenerics(self.add_lifetime(ctx.trait_lifetime.clone()).clone())
        } else {
            // no wrapper at all, just lifetime
            let mut generics = syn::Generics::default();
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                    ctx.trait_lifetime.clone(),
                )));
            TypeGenerics(generics)
        }
    }
}

impl<'src> Compile<'src, FieldsIter, TypeGenerics> for Enum<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> TypeGenerics {
        if self.any_wrapper() {
            // contains wrapper, but has regular, add lifetime
            TypeGenerics(self.add_lifetime(ctx.trait_lifetime.clone()).clone())
        } else {
            // no wrapper at all, just lifetime
            let mut generics = syn::Generics::default();
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                    ctx.trait_lifetime.clone(),
                )));
            TypeGenerics(generics)
        }
    }
}

target! {
    /// Type head for the generated iterator
    pub struct TypeHead
}

impl<'src> Compile<'src, FieldsIter, TypeHead> for Struct<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> TypeHead {
        let iter_name: Name = self.compile(ctx);
        let generics: TypeGenerics = self.compile(ctx);
        TypeHead(quote! {
            #iter_name #generics
        })
    }
}

impl<'src> Compile<'src, FieldsIter, TypeHead> for Enum<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> TypeHead {
        let iter_name: Name = self.compile(ctx);
        let generics: TypeGenerics = self.compile(ctx);
        TypeHead(quote! {
            #iter_name #generics
        })
    }
}
