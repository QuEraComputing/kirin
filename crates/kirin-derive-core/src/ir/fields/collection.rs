use proc_macro2::TokenStream;
use quote::quote;

use crate::misc::{is_type, is_type_in};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Collection {
    #[default]
    Single,
    Vec,
    Option,
}

impl Collection {
    pub fn from_type<I>(ty: &syn::Type, name: &I) -> Option<Self>
    where
        I: ?Sized,
        syn::Ident: PartialEq<I> + PartialEq<str>,
    {
        if is_type(ty, name) {
            Some(Collection::Single)
        } else if is_type_in(ty, name, |seg| seg.ident == "Vec") {
            Some(Collection::Vec)
        } else if is_type_in(ty, name, |seg| seg.ident == "Option") {
            Some(Collection::Option)
        } else {
            None
        }
    }

    /// Wraps a base type TokenStream with the appropriate collection wrapper.
    ///
    /// - `Single` returns the base type unchanged
    /// - `Vec` returns `Vec<base>`
    /// - `Option` returns `Option<base>`
    pub fn wrap_type(&self, base: TokenStream) -> TokenStream {
        match self {
            Collection::Single => base,
            Collection::Vec => quote! { Vec<#base> },
            Collection::Option => quote! { Option<#base> },
        }
    }

    /// Wraps a parser TokenStream with the appropriate collection combinator.
    ///
    /// - `Single` returns the parser unchanged
    /// - `Vec` returns `parser.repeated().collect()`
    /// - `Option` returns `parser.or_not()`
    pub fn wrap_parser(&self, parser: TokenStream) -> TokenStream {
        match self {
            Collection::Single => parser,
            Collection::Vec => quote! { #parser.repeated().collect() },
            Collection::Option => quote! { #parser.or_not() },
        }
    }
}
