use proc_macro2::TokenStream;
use quote::quote;

use crate::misc::{is_type, is_type_in};

/// How a field's base type is wrapped in a collection.
///
/// A field typed `Vec<SSAValue>` is classified as `Argument` with
/// `Collection::Vec`. The collection affects generated parser and
/// constructor code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Collection {
    /// Bare type, no wrapping.
    #[default]
    Single,
    /// Wrapped in `Vec<T>`.
    Vec,
    /// Wrapped in `Option<T>`.
    Option,
}

impl Collection {
    /// Detect collection wrapping from a `syn::Type`.
    ///
    /// Returns `Some(Single)` if `ty` matches `name` directly, `Some(Vec)` if
    /// it matches `Vec<name>`, `Some(Option)` for `Option<name>`, or `None`.
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

    /// Wraps a base type token in the collection (e.g., `Vec<base>`).
    pub fn wrap_type(&self, base: TokenStream) -> TokenStream {
        match self {
            Collection::Single => base,
            Collection::Vec => quote! { Vec<#base> },
            Collection::Option => quote! { Option<#base> },
        }
    }

    /// Wraps a parser expression for this collection kind.
    pub fn wrap_parser(&self, parser: TokenStream) -> TokenStream {
        match self {
            Collection::Single => parser,
            Collection::Vec => quote! { #parser.repeated().collect() },
            Collection::Option => quote! { #parser.or_not() },
        }
    }
}
