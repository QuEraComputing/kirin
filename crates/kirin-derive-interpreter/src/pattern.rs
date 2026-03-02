use kirin_derive_core::prelude::*;
use kirin_derive_core::tokens::FieldPatternTokens;
use quote::quote;

/// Build field destructuring pattern tokens for a statement.
///
/// Shared between `Interpretable` and `CallSemantics` derive macros.
pub(crate) fn build_pattern<L: ir::Layout>(statement: &ir::Statement<L>) -> FieldPatternTokens {
    let mut fields = Vec::new();
    if let Some(wrapper) = &statement.wraps {
        fields.push(wrapper.field.clone());
    }
    for f in statement.iter_all_fields() {
        fields.push(ir::fields::FieldIndex::new(f.ident.clone(), f.index));
    }
    fields.sort_by_key(|field| field.index);

    if fields.is_empty() {
        return FieldPatternTokens::new(false, Vec::new());
    }
    let named = fields.iter().any(|f| f.ident.is_some());
    let names: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .map(|f| {
            let name = f.name();
            quote! { #name }
        })
        .collect();
    FieldPatternTokens::new(named, names)
}
