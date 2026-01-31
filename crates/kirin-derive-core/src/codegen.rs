//! Code generation utilities for derive macros.
//!
//! This module provides common helpers for generating code patterns
//! that are frequently needed in derive macro implementations.

use proc_macro2::Span;

/// Generates a sequence of identifiers for tuple fields.
///
/// Given a prefix and count, generates identifiers like `f0`, `f1`, `f2`, etc.
///
/// # Example
/// ```ignore
/// let names = tuple_field_idents("f", 3);
/// // Produces: [f0, f1, f2]
/// ```
pub fn tuple_field_idents(prefix: &str, count: usize) -> Vec<syn::Ident> {
    (0..count)
        .map(|i| syn::Ident::new(&format!("{}{}", prefix, i), Span::call_site()))
        .collect()
}

/// Generates renamed identifiers from named fields.
///
/// Given a prefix and a list of field identifiers, generates renamed versions
/// like `s_field1`, `s_field2`, etc.
///
/// # Example
/// ```ignore
/// let fields = vec![ident("x"), ident("y")];
/// let renamed = renamed_field_idents("s_", &fields);
/// // Produces: [s_x, s_y]
/// ```
pub fn renamed_field_idents(prefix: &str, fields: &[syn::Ident]) -> Vec<syn::Ident> {
    fields
        .iter()
        .map(|f| syn::Ident::new(&format!("{}{}", prefix, f), Span::call_site()))
        .collect()
}
