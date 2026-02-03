//! Shared bound generation for derive macro implementations.
//!
//! This module provides `BoundsBuilder` which consolidates the duplicated
//! where clause bound generation logic across generators.

use proc_macro2::TokenStream;

/// Builder for generating where clause bounds.
///
/// This consolidates the common pattern of generating trait bounds
/// for value types containing type parameters.
pub struct BoundsBuilder<'a> {
    /// Path to the kirin-chumsky crate
    crate_path: &'a syn::Path,
    /// Path to the kirin IR crate
    ir_path: &'a syn::Path,
}

impl<'a> BoundsBuilder<'a> {
    /// Creates a new bounds builder.
    pub fn new(crate_path: &'a syn::Path, ir_path: &'a syn::Path) -> Self {
        Self {
            crate_path,
            ir_path,
        }
    }

    /// Generates `HasParser<'tokens, 'src> + 'tokens` bounds for the given types.
    ///
    /// Used by: parser, ast
    pub fn has_parser_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect()
    }

    /// Generates `HasDialectParser<'tokens, 'src, L>` bounds for wrapper types.
    ///
    /// This is used for wrapper types where we forward the Language parameter through
    /// to nested blocks.
    ///
    /// The `language_type` parameter specifies what type to use as the Language:
    /// - For `HasDialectParser` impl: use `quote! { Language }`
    /// - For `HasParser` impl: use the concrete dialect type (e.g., `quote! { MyDialect #ty_generics }`)
    ///
    /// Used by: parser impl_gen
    pub fn has_dialect_parser_bounds(
        &self,
        types: &[syn::Type],
        language_type: &TokenStream,
    ) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src, #language_type> })
            .collect()
    }

    /// Generates the TypeLattice bound: `TypeLattice: HasParser<'tokens, 'src> + 'tokens`.
    pub fn type_lattice_has_parser_bound(&self, type_lattice: &syn::Path) -> syn::WherePredicate {
        let crate_path = self.crate_path;
        syn::parse_quote! { #type_lattice: #crate_path::HasParser<'tokens, 'src> + 'tokens }
    }

    /// Generates the Language: Dialect + 'tokens bound.
    pub fn language_dialect_bound(&self) -> syn::WherePredicate {
        let ir_path = self.ir_path;
        syn::parse_quote! { Language: #ir_path::Dialect + 'tokens }
    }

    /// Generates `EmitIR` bounds for the given types.
    ///
    /// For each type T, generates:
    /// - `T: HasParser<'tokens, 'src> + 'tokens`
    /// - `<T as HasParser<'tokens, 'src>>::Output: EmitIR<Language, Output = T>`
    ///
    /// Used by: emit_ir
    pub fn emit_ir_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .flat_map(|ty| {
                vec![
                    syn::parse_quote! {
                        #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens
                    },
                    syn::parse_quote! {
                        <#ty as #crate_path::HasParser<'tokens, 'src>>::Output: #crate_path::EmitIR<Language, Output = #ty>
                    },
                ]
            })
            .collect()
    }

    /// Generates `PrettyPrint` bounds for the given types.
    ///
    /// For each type T, generates: `T: PrettyPrint<DialectType>`
    ///
    /// Used by: pretty_print
    pub fn pretty_print_bounds(
        &self,
        types: &[syn::Type],
        dialect_type: &TokenStream,
        prettyless_path: &syn::Path,
    ) -> Vec<syn::WherePredicate> {
        types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #prettyless_path::PrettyPrint<#dialect_type> })
            .collect()
    }
}
