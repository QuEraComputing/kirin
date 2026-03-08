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

/// Merge two optional where clauses into one, concatenating their predicates.
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

/// Remove duplicate types from the list, comparing by token representation.
pub fn deduplicate_types(types: &mut Vec<syn::Type>) {
    let mut seen = std::collections::HashSet::new();
    types.retain(|ty| {
        let key = quote!(#ty).to_string();
        seen.insert(key)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_where_clauses_both_none() {
        assert!(combine_where_clauses(None, None).is_none());
    }

    #[test]
    fn combine_where_clauses_first_some() {
        let wc: syn::WhereClause = syn::parse_quote!(where T: Clone);
        let result = combine_where_clauses(Some(&wc), None);
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.predicates.len(), 1);
    }

    #[test]
    fn combine_where_clauses_second_some() {
        let wc: syn::WhereClause = syn::parse_quote!(where T: Debug);
        let result = combine_where_clauses(None, Some(&wc));
        assert!(result.is_some());
    }

    #[test]
    fn combine_where_clauses_both_some() {
        let wc1: syn::WhereClause = syn::parse_quote!(where T: Clone);
        let wc2: syn::WhereClause = syn::parse_quote!(where U: Debug);
        let result = combine_where_clauses(Some(&wc1), Some(&wc2));
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.predicates.len(), 2);
    }

    #[test]
    fn deduplicate_types_removes_duplicates() {
        let mut types: Vec<syn::Type> = vec![
            syn::parse_quote!(i32),
            syn::parse_quote!(String),
            syn::parse_quote!(i32),
            syn::parse_quote!(bool),
            syn::parse_quote!(String),
        ];
        deduplicate_types(&mut types);
        assert_eq!(types.len(), 3);
    }

    #[test]
    fn deduplicate_types_empty() {
        let mut types: Vec<syn::Type> = vec![];
        deduplicate_types(&mut types);
        assert!(types.is_empty());
    }

    #[test]
    fn deduplicate_types_all_unique() {
        let mut types: Vec<syn::Type> = vec![
            syn::parse_quote!(i32),
            syn::parse_quote!(String),
            syn::parse_quote!(bool),
        ];
        deduplicate_types(&mut types);
        assert_eq!(types.len(), 3);
    }

    #[test]
    fn deduplicate_types_preserves_order() {
        let mut types: Vec<syn::Type> = vec![
            syn::parse_quote!(bool),
            syn::parse_quote!(i32),
            syn::parse_quote!(bool),
        ];
        deduplicate_types(&mut types);
        assert_eq!(types.len(), 2);
        // First occurrence is preserved
        assert_eq!(quote!(#(#types),*).to_string(), "bool , i32");
    }
}
