use proc_macro2::Span;
use quote::quote;

/// Generates a sequence of identifiers for tuple fields.
///
/// Given a prefix and count, generates identifiers like `f0`, `f1`, `f2`, etc.
pub(super) fn tuple_field_idents(prefix: &str, count: usize) -> Vec<syn::Ident> {
    (0..count)
        .map(|i| syn::Ident::new(&format!("{}{}", prefix, i), Span::call_site()))
        .collect()
}

/// Generates renamed identifiers from named fields.
///
/// Given a prefix and a list of field identifiers, generates renamed versions
/// like `s_field1`, `s_field2`, etc.
pub(super) fn renamed_field_idents(prefix: &str, fields: &[syn::Ident]) -> Vec<syn::Ident> {
    fields
        .iter()
        .map(|f| syn::Ident::new(&format!("{}{}", prefix, f), Span::call_site()))
        .collect()
}

/// Combines two optional where clauses into one.
///
/// This is a common pattern when building impls that need to combine
/// the original type's where clause with additional generated bounds.
///
/// # Example
///
/// ```ignore
/// let combined = combine_where_clauses(orig_where.as_ref(), impl_where.as_ref());
/// ```
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

/// Deduplicates a list of types by their token representation.
///
/// This is useful when collecting types for trait bounds, where the same
/// type might appear multiple times from different fields.
///
/// # Example
///
/// ```ignore
/// let mut types = vec![parse_quote!(T), parse_quote!(U), parse_quote!(T)];
/// deduplicate_types(&mut types);
/// // types is now [T, U]
/// ```
pub fn deduplicate_types(types: &mut Vec<syn::Type>) {
    let mut seen = std::collections::HashSet::new();
    types.retain(|ty| {
        let key = quote!(#ty).to_string();
        seen.insert(key)
    });
}
