use super::utils::{renamed_field_idents, tuple_field_idents};

/// Captures field variable names for use in destructuring patterns and
/// generated code bodies.
///
/// For named fields, preserves original names with an optional prefix.
/// For tuple fields, generates `{prefix}0`, `{prefix}1`, etc.
#[derive(Debug, Clone)]
pub struct FieldBindings {
    /// Whether the fields are positional (tuple) or named (struct).
    pub is_tuple: bool,
    /// Total number of fields.
    pub field_count: usize,
    /// Prefixed idents used in generated code bindings.
    pub field_idents: Vec<syn::Ident>,
    /// Original field names before prefixing (empty for tuple fields).
    pub original_field_names: Vec<syn::Ident>,
}

impl FieldBindings {
    /// Create bindings for tuple fields: `{prefix}0`, `{prefix}1`, etc.
    pub fn tuple(prefix: &str, count: usize) -> Self {
        Self {
            is_tuple: true,
            field_count: count,
            field_idents: tuple_field_idents(prefix, count),
            original_field_names: Vec::new(),
        }
    }

    /// Create bindings for named fields, prefixing each name with `{prefix}_`.
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

    pub fn is_empty(&self) -> bool {
        self.field_count == 0
    }

    /// Generate a new set of idents with a different prefix, preserving the field style.
    pub fn renamed(&self, prefix: &str) -> Vec<syn::Ident> {
        if self.is_tuple {
            tuple_field_idents(prefix, self.field_count)
        } else {
            renamed_field_idents(&format!("{}_", prefix), &self.original_field_names)
        }
    }

    /// Clone these bindings with a new prefix applied to all idents.
    pub fn with_prefix(&self, prefix: &str) -> Self {
        Self {
            is_tuple: self.is_tuple,
            field_count: self.field_count,
            field_idents: self.renamed(prefix),
            original_field_names: self.original_field_names.clone(),
        }
    }
}
