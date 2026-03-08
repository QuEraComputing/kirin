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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_type_single_match() {
        let ty: syn::Type = syn::parse_str("SSAValue").unwrap();
        assert_eq!(
            Collection::from_type(&ty, "SSAValue"),
            Some(Collection::Single)
        );
    }

    #[test]
    fn from_type_vec_match() {
        let ty: syn::Type = syn::parse_str("Vec<SSAValue>").unwrap();
        assert_eq!(
            Collection::from_type(&ty, "SSAValue"),
            Some(Collection::Vec)
        );
    }

    #[test]
    fn from_type_option_match() {
        let ty: syn::Type = syn::parse_str("Option<Block>").unwrap();
        assert_eq!(
            Collection::from_type(&ty, "Block"),
            Some(Collection::Option)
        );
    }

    #[test]
    fn from_type_no_match() {
        let ty: syn::Type = syn::parse_str("String").unwrap();
        assert_eq!(Collection::from_type(&ty, "SSAValue"), None);
    }

    #[test]
    fn from_type_nested_generic_no_match() {
        // HashMap<String, SSAValue> should not match Vec or Option
        let ty: syn::Type = syn::parse_str("HashMap<String, SSAValue>").unwrap();
        assert_eq!(Collection::from_type(&ty, "SSAValue"), None);
    }

    #[test]
    fn from_type_reference_no_match() {
        let ty: syn::Type = syn::parse_str("&SSAValue").unwrap();
        assert_eq!(Collection::from_type(&ty, "SSAValue"), None);
    }

    #[test]
    fn wrap_type_single() {
        let base = quote! { i32 };
        let wrapped = Collection::Single.wrap_type(base);
        assert_eq!(wrapped.to_string(), "i32");
    }

    #[test]
    fn wrap_type_vec() {
        let base = quote! { i32 };
        let wrapped = Collection::Vec.wrap_type(base);
        assert!(wrapped.to_string().contains("Vec"));
    }

    #[test]
    fn wrap_type_option() {
        let base = quote! { Block };
        let wrapped = Collection::Option.wrap_type(base);
        assert!(wrapped.to_string().contains("Option"));
    }

    #[test]
    fn wrap_parser_single_passthrough() {
        let parser = quote! { my_parser() };
        let wrapped = Collection::Single.wrap_parser(parser.clone());
        assert_eq!(wrapped.to_string(), parser.to_string());
    }

    #[test]
    fn wrap_parser_vec_adds_repeated() {
        let parser = quote! { my_parser() };
        let wrapped = Collection::Vec.wrap_parser(parser);
        let s = wrapped.to_string();
        assert!(s.contains("repeated"), "Expected 'repeated' in: {s}");
        assert!(s.contains("collect"), "Expected 'collect' in: {s}");
    }

    #[test]
    fn wrap_parser_option_adds_or_not() {
        let parser = quote! { my_parser() };
        let wrapped = Collection::Option.wrap_parser(parser);
        let s = wrapped.to_string();
        assert!(s.contains("or_not"), "Expected 'or_not' in: {s}");
    }

    #[test]
    fn default_is_single() {
        assert_eq!(Collection::default(), Collection::Single);
    }
}
