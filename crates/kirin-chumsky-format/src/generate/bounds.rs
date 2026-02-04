//! Shared bound generation for derive macro implementations.
//!
//! This module provides `BoundsBuilder` which consolidates the duplicated
//! where clause bound generation logic across generators.

/// Builder for generating where clause bounds.
///
/// This consolidates the common pattern of generating trait bounds
/// for value types containing type parameters.
pub struct BoundsBuilder<'a> {
    /// Path to the kirin-chumsky crate
    crate_path: &'a syn::Path,
}

impl<'a> BoundsBuilder<'a> {
    /// Creates a new bounds builder.
    pub fn new(crate_path: &'a syn::Path, _ir_path: &'a syn::Path) -> Self {
        Self { crate_path }
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

    /// Generates `HasDialectParser<'tokens, 'src>` bounds for wrapper types.
    ///
    /// With GAT, the trait no longer has a Language type parameter.
    /// The Language is passed as a method type parameter instead.
    ///
    /// Used by: parser impl_gen, ast generation
    pub fn has_dialect_parser_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect()
    }

    /// Generates the TypeLattice bound: `TypeLattice: HasParser<'tokens, 'src> + 'tokens`.
    pub fn type_lattice_has_parser_bound(&self, type_lattice: &syn::Path) -> syn::WherePredicate {
        let crate_path = self.crate_path;
        syn::parse_quote! { #type_lattice: #crate_path::HasParser<'tokens, 'src> + 'tokens }
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
}
