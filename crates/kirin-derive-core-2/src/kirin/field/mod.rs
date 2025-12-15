use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::data::{Compile, Dialect, FromContext};

mod context;
mod enum_impl;
mod extra;
mod iter;
mod struct_impl;

pub use context::FieldsIter;

enum FieldImpl<'a, 'src> {
    Struct(struct_impl::StructImpl<'a, 'src>),
    Enum(enum_impl::EnumImpl<'src>),
}

impl<'a, 'src> Compile<'src, context::FieldsIter, Dialect<'src, context::FieldsIter>>
    for FieldImpl<'a, 'src>
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

impl ToTokens for FieldImpl<'_, '_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            FieldImpl::Struct(s) => s.to_tokens(tokens),
            FieldImpl::Enum(e) => e.to_tokens(tokens),
        }
    }
}

impl FieldsIter {
    /// Emit the field iterator implementation for the given derive input
    pub fn emit(&self, input: &syn::DeriveInput) -> TokenStream {
        self.emit_inner(input)
            .unwrap_or_else(|e| e.to_compile_error())
    }

    pub fn print(&self, input: &syn::DeriveInput) -> String {
        let file = syn::parse_file(&self.emit(input).to_string()).unwrap();
        prettyplease::unparse(&file)
    }

    fn emit_inner(&self, input: &syn::DeriveInput) -> syn::Result<TokenStream> {
        let dialect = Dialect::from_context(self, input)?;
        println!("Dialect: {:#?}", dialect);
        let fi = FieldImpl::compile(self, &dialect)?;
        Ok(fi.to_token_stream())
    }
}

#[cfg(test)]
mod tests;
