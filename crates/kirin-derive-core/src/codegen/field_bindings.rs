use super::utils::{renamed_field_idents, tuple_field_idents};

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
    /// For named-style: generated names like `f_fieldname`.
    pub field_idents: Vec<syn::Ident>,
    /// The original field names (for named-style only).
    /// Used to generate patterns like `{ field_name: binding_name }`.
    pub original_field_names: Vec<syn::Ident>,
}

impl FieldBindings {
    /// Creates field bindings for a tuple-style struct/variant.
    pub fn tuple(prefix: &str, count: usize) -> Self {
        Self {
            is_tuple: true,
            field_count: count,
            field_idents: tuple_field_idents(prefix, count),
            original_field_names: Vec::new(),
        }
    }

    /// Creates field bindings for a named struct/variant.
    ///
    /// Generates binding names with the given prefix (e.g., `f_fieldname`).
    pub fn named(prefix: &str, fields: Vec<syn::Ident>) -> Self {
        let count = fields.len();
        let prefixed = renamed_field_idents(&format!("{}_", prefix), &fields);
        Self {
            is_tuple: false,
            field_count: count,
            field_idents: prefixed,
            original_field_names: fields,
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
            renamed_field_idents(&format!("{}_", prefix), &self.original_field_names)
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
            original_field_names: self.original_field_names.clone(),
        }
    }
}
