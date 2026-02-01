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

/// Field binding information for code generation.
///
/// This struct captures all the identifiers and patterns needed to work with
/// struct/variant fields in generated code.
#[derive(Debug, Clone)]
pub struct FieldBindings {
    /// Whether this is a tuple-style (positional) or named struct/variant.
    pub is_tuple: bool,
    /// The field count (for tuple-style).
    pub field_count: usize,
    /// The field identifiers to use in patterns and expressions.
    /// For tuple-style: generated names like `f0`, `f1`.
    /// For named-style: the actual field identifiers.
    pub field_idents: Vec<syn::Ident>,
}

impl FieldBindings {
    /// Creates field bindings for a tuple-style struct/variant.
    pub fn tuple(prefix: &str, count: usize) -> Self {
        Self {
            is_tuple: true,
            field_count: count,
            field_idents: tuple_field_idents(prefix, count),
        }
    }

    /// Creates field bindings for a named struct/variant.
    pub fn named(fields: Vec<syn::Ident>) -> Self {
        let count = fields.len();
        Self {
            is_tuple: false,
            field_count: count,
            field_idents: fields,
        }
    }

    /// Returns true if there are no fields.
    pub fn is_empty(&self) -> bool {
        self.field_count == 0
    }

    /// Generates renamed identifiers with the given prefix.
    ///
    /// For tuple-style, generates `prefix0`, `prefix1`, etc.
    /// For named-style, generates `prefix_fieldname` for each field.
    pub fn renamed(&self, prefix: &str) -> Vec<syn::Ident> {
        if self.is_tuple {
            tuple_field_idents(prefix, self.field_count)
        } else {
            renamed_field_idents(prefix, &self.field_idents)
        }
    }

    /// Creates a new FieldBindings with renamed identifiers.
    ///
    /// This is useful when you need a second set of bindings (e.g., for PartialEq).
    pub fn with_prefix(&self, prefix: &str) -> Self {
        Self {
            is_tuple: self.is_tuple,
            field_count: self.field_count,
            field_idents: self.renamed(prefix),
        }
    }
}
