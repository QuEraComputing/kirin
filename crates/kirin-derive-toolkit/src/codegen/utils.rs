use proc_macro2::Span;
use quote::quote;

pub(super) fn tuple_field_idents(prefix: &str, count: usize) -> Vec<syn::Ident> {
    (0..count)
        .map(|i| syn::Ident::new(&format!("{}{}", prefix, i), Span::call_site()))
        .collect()
}

pub(super) fn renamed_field_idents(prefix: &str, fields: &[syn::Ident]) -> Vec<syn::Ident> {
    fields
        .iter()
        .map(|f| syn::Ident::new(&format!("{}{}", prefix, f), Span::call_site()))
        .collect()
}

pub fn combine_where_clauses(
    a: Option<&syn::WhereClause>,
    b: Option<&syn::WhereClause>,
) -> Option<syn::WhereClause> {
    match (a, b) {
        (Some(orig), Some(other)) => {
            let mut combined = orig.clone();
            combined.predicates.extend(other.predicates.iter().cloned());
            Some(combined)
        }
        (Some(wc), None) | (None, Some(wc)) => Some(wc.clone()),
        (None, None) => None,
    }
}

pub fn deduplicate_types(types: &mut Vec<syn::Type>) {
    let mut seen = std::collections::HashSet::new();
    types.retain(|ty| {
        let key = quote!(#ty).to_string();
        seen.insert(key)
    });
}
