//! Shared bound generation for derive macro implementations.

/// Builder for generating where clause bounds.
pub struct BoundsBuilder<'a> {
    /// Path to the kirin-chumsky crate
    crate_path: &'a syn::Path,
}

impl<'a> BoundsBuilder<'a> {
    /// Creates a new bounds builder.
    pub fn new(crate_path: &'a syn::Path) -> Self {
        Self { crate_path }
    }

    /// Generates `HasParser<'t> + 't` bounds for the given types.
    pub fn has_parser_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasParser<'t> + 't })
            .collect()
    }

    /// Generates `HasDialectParser<'t>` bounds for wrapper types.
    pub fn has_dialect_parser_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasDialectParser<'t> })
            .collect()
    }

    /// Generates the IR type bound: `IrType: HasParser<'t> + 't`.
    pub fn ir_type_has_parser_bound(&self, ir_type: &syn::Path) -> syn::WherePredicate {
        let crate_path = self.crate_path;
        syn::parse_quote! { #ir_type: #crate_path::HasParser<'t> + 't }
    }

    /// Generates `EmitIR` bounds for the given types.
    pub fn emit_ir_bounds(&self, types: &[syn::Type]) -> Vec<syn::WherePredicate> {
        let crate_path = self.crate_path;
        types
            .iter()
            .flat_map(|ty| {
                vec![
                    syn::parse_quote! {
                        #ty: #crate_path::HasParser<'t> + 't
                    },
                    syn::parse_quote! {
                        <#ty as #crate_path::HasParser<'t>>::Output: #crate_path::EmitIR<Language, Output = #ty>
                    },
                ]
            })
            .collect()
    }
}
