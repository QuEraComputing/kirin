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

impl<'src> Compile<'src, Struct<'src, FieldsIter>, TypeGenerics> for FieldsIter {
    fn compile(&self, node: &Struct<'src, FieldsIter>) -> TypeGenerics {
        if node.is_wrapper() {
            TypeGenerics(node.add_lifetime(self.trait_lifetime.clone()).clone())
        } else {
            // no wrapper at all, just lifetime
            let mut generics = syn::Generics::default();
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                    self.trait_lifetime.clone(),
                )));
            TypeGenerics(generics)
        }
    }
}

impl<'src> Compile<'src, Enum<'src, FieldsIter>, TypeGenerics> for FieldsIter {
    fn compile(&self, node: &Enum<'src, FieldsIter>) -> TypeGenerics {
        if node.any_wrapper() {
            // contains wrapper, but has regular, add lifetime
            TypeGenerics(node.add_lifetime(self.trait_lifetime.clone()).clone())
        } else {
            // no wrapper at all, just lifetime
            let mut generics = syn::Generics::default();
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                    self.trait_lifetime.clone(),
                )));
            TypeGenerics(generics)
        }
    }
}

target! {
    /// Type head for the generated iterator
    pub struct TypeHead
}

impl<'src> Compile<'src, Struct<'src, FieldsIter>, TypeHead> for FieldsIter {
    fn compile(&self, node: &Struct<'src, FieldsIter>) -> TypeHead {
        let iter_name: Name = self.compile(node);
        let generics: TypeGenerics = self.compile(node);
        TypeHead(quote! {
            #iter_name #generics
        })
    }
}

impl<'src> Compile<'src, Enum<'src, FieldsIter>, TypeHead> for FieldsIter {
    fn compile(&self, node: &Enum<'src, FieldsIter>) -> TypeHead {
        let iter_name: Name = self.compile(node);
        let generics: TypeGenerics = self.compile(node);
        TypeHead(quote! {
            #iter_name #generics
        })
    }
}
