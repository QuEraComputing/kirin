//! Generics utilities for code generation.
//!
//! This module provides utilities for building generic parameters used in
//! generated AST types and parser implementations.

use proc_macro2::Span;

/// Builder for AST generics with 'tokens, 'src, and Language parameters.
pub struct GenericsBuilder<'a> {
    ir_path: &'a syn::Path,
}

impl<'a> GenericsBuilder<'a> {
    /// Creates a new generics builder.
    pub fn new(ir_path: &'a syn::Path) -> Self {
        Self { ir_path }
    }

    /// Builds generics with 'tokens, 'src: 'tokens lifetimes only.
    ///
    /// This is used for the original type's `HasDialectParser` impl where
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
    /// AST types only require `Language: Dialect`, not `HasDialectParser`.
    pub fn with_language(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);
        let ir_path = self.ir_path;

        // Add Language type parameter if not present
        // Language only needs the Dialect bound (AST types don't implement HasDialectParser)
        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let mut lang_param = syn::TypeParam::from(lang_ident);
            lang_param
                .bounds
                .push(syn::parse_quote!(#ir_path::Dialect));
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }

    /// Builds generics with 'tokens, 'src: 'tokens lifetimes and Language type parameter without bounds.
    ///
    /// This is used for `HasDialectParser` impl where the `Language: Dialect` bound
    /// is specified in the where clause instead of on the type parameter.
    pub fn with_language_unbounded(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);

        // Add Language type parameter without any bounds
        // The Dialect bound will be added in the where clause
        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let lang_param = syn::TypeParam::from(lang_ident);
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn format_generics(generics: &syn::Generics) -> String {
        let tokens = quote! { #generics };
        tokens.to_string()
    }

    #[test]
    fn test_with_lifetimes_empty() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base = syn::Generics::default();
        let result = builder.with_lifetimes(&base);

        insta::assert_snapshot!("with_lifetimes_empty", format_generics(&result));
    }

    #[test]
    fn test_with_lifetimes_existing_type_param() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base: syn::Generics = syn::parse_quote!(<T: Clone>);
        let result = builder.with_lifetimes(&base);

        insta::assert_snapshot!("with_lifetimes_existing_type", format_generics(&result));
    }

    #[test]
    fn test_with_language_empty() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base = syn::Generics::default();
        let result = builder.with_language(&base);

        insta::assert_snapshot!("with_language_empty", format_generics(&result));
    }

    #[test]
    fn test_with_language_custom_ir_path() {
        let ir_path: syn::Path = syn::parse_quote!(my_kirin);
        let builder = GenericsBuilder::new(&ir_path);

        let base = syn::Generics::default();
        let result = builder.with_language(&base);

        insta::assert_snapshot!("with_language_custom_ir", format_generics(&result));
    }

    #[test]
    fn test_with_language_existing_type_param() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base: syn::Generics = syn::parse_quote!(<T: CompileTimeValue>);
        let result = builder.with_language(&base);

        insta::assert_snapshot!("with_language_existing_type", format_generics(&result));
    }
}
