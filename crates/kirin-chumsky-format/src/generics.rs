//! Generics utilities for code generation.
//!
//! This module re-exports `GenericsBuilder` from `kirin-derive-core` for convenience.

// Re-export GenericsBuilder from the core crate
pub use kirin_derive_core::codegen::GenericsBuilder;

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
