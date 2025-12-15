use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::data::{Compile, Dialect, FromContext};

mod context;
mod enum_impl;
mod extra;
mod iter;
mod struct_impl;

pub use context::FieldsIter;

enum FieldImpl<'src> {
    Struct(struct_impl::StructImpl<'src>),
    Enum(enum_impl::EnumImpl<'src>),
}

impl<'src> Compile<'src, context::FieldsIter, Dialect<'src, context::FieldsIter>>
    for FieldImpl<'src>
{
    fn compile(
        ctx: &'src context::FieldsIter,
        node: &'src Dialect<'src, context::FieldsIter>,
    ) -> syn::Result<Self> {
        match node {
            Dialect::Struct(s) => Ok(FieldImpl::Struct(struct_impl::StructImpl::compile(ctx, s)?)),
            Dialect::Enum(e) => Ok(FieldImpl::Enum(enum_impl::EnumImpl::compile(ctx, e)?)),
        }
    }
}

impl ToTokens for FieldImpl<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            FieldImpl::Struct(s) => s.to_tokens(tokens),
            FieldImpl::Enum(e) => e.to_tokens(tokens),
        }
    }
}

impl FieldsIter {
    /// Emit the field iterator implementation for the given derive input
    pub fn emit(&self, input: &syn::DeriveInput) -> Result<TokenStream, syn::Error> {
        let dialect = Dialect::from_context(self, input)?;
        let fi = FieldImpl::compile(self, &dialect)?;
        Ok(fi.to_token_stream())
    }
}

#[cfg(test)]
mod tests;
