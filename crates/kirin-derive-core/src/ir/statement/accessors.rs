use std::collections::HashMap;

use super::super::{fields::*, layout::Layout};
use super::definition::Statement;

impl<L: Layout> Statement<L> {
    // =========================================================================
    // Field Iteration Methods
    // =========================================================================

    /// Iterates over all fields in declaration order.
    pub fn iter_all_fields(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields.iter()
    }

    /// Iterates over argument fields (SSAValue).
    pub fn arguments(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Argument)
    }

    /// Iterates over result fields (ResultValue).
    pub fn results(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Result)
    }

    /// Iterates over block fields.
    pub fn blocks(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Block)
    }

    /// Iterates over successor fields.
    pub fn successors(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Successor)
    }

    /// Iterates over region fields.
    pub fn regions(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Region)
    }

    /// Iterates over compile-time value fields.
    pub fn values(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Value)
    }

    // =========================================================================
    // Field Query Methods
    // =========================================================================

    /// Returns the total count of fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Collects all named field identifiers in declaration order.
    ///
    /// Returns identifiers only for fields that have names (not tuple fields).
    pub fn named_field_idents(&self) -> Vec<syn::Ident> {
        self.fields.iter().filter_map(|f| f.ident.clone()).collect()
    }

    /// Returns true if all fields are unnamed (tuple-style).
    pub fn is_tuple_style(&self) -> bool {
        self.fields.iter().all(|f| f.ident.is_none())
    }

    /// Builds a map from field name to field index.
    ///
    /// Only includes fields that have names.
    pub fn field_name_to_index(&self) -> HashMap<String, usize> {
        self.fields
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|id| (id.to_string(), f.index)))
            .collect()
    }

    /// Creates field bindings for use in pattern matching and code generation.
    ///
    /// For tuple-style structs/variants, generates bindings like `f0`, `f1`, etc.
    /// For named structs/variants, generates bindings like `f_fieldname`.
    ///
    /// The `prefix` is used for generating unique binding variable names.
    pub fn field_bindings(&self, prefix: &str) -> crate::codegen::FieldBindings {
        if self.is_tuple_style() {
            crate::codegen::FieldBindings::tuple(prefix, self.field_count())
        } else {
            crate::codegen::FieldBindings::named(prefix, self.named_field_idents())
        }
    }

    /// Returns a clone of all fields (already in declaration order).
    pub fn collect_fields(&self) -> Vec<FieldInfo<L>> {
        self.fields.clone()
    }
}
