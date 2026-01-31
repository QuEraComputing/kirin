//! Generics utilities for code generation.
//!
//! This module provides utilities for building generic parameters used in
//! generated AST types and parser implementations.

use proc_macro2::Span;

/// Builder for AST generics with 'tokens, 'src, and Language parameters.
pub struct GenericsBuilder<'a> {
    crate_path: &'a syn::Path,
}

impl<'a> GenericsBuilder<'a> {
    /// Creates a new generics builder.
    pub fn new(crate_path: &'a syn::Path) -> Self {
        Self { crate_path }
    }

    /// Builds generics with 'tokens, 'src: 'tokens lifetimes only.
    ///
    /// This is used for the original type's `HasRecursiveParser` impl where
    /// the type is its own Language parameter.
    pub fn with_lifetimes(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = base.clone();

        // Add 'tokens lifetime at the beginning if not present
        let tokens_lt = syn::Lifetime::new("'tokens", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
        {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt.clone())),
            );
        }

        // Add 'src: 'tokens lifetime after 'tokens if not present
        let src_lt = syn::Lifetime::new("'src", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "src"))
        {
            let mut src_param = syn::LifetimeParam::new(src_lt);
            src_param.bounds.push(tokens_lt);
            generics
                .params
                .insert(1, syn::GenericParam::Lifetime(src_param));
        }

        generics
    }

    /// Builds generics with 'tokens, 'src: 'tokens lifetimes and Language type parameter.
    ///
    /// This is used for AST types and their trait implementations.
    pub fn with_language(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);
        let crate_path = self.crate_path;

        // Add Language type parameter if not present
        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let mut lang_param = syn::TypeParam::from(lang_ident);
            lang_param
                .bounds
                .push(syn::parse_quote!(#crate_path::LanguageParser<'tokens, 'src>));
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }
}
