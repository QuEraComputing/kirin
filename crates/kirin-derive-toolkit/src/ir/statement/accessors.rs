use std::collections::HashMap;

use super::super::{fields::*, layout::Layout};
use super::definition::Statement;

impl<L: Layout> Statement<L> {
    /// Iterates all fields regardless of category.
    pub fn iter_all_fields(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields.iter()
    }

    /// Iterates fields classified as [`FieldCategory::Argument`] (SSAValue types).
    pub fn arguments(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Argument)
    }

    /// Iterates fields classified as [`FieldCategory::Result`] (ResultValue types).
    pub fn results(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Result)
    }

    /// Iterates fields classified as [`FieldCategory::Block`].
    pub fn blocks(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Block)
    }

    /// Iterates fields classified as [`FieldCategory::Successor`].
    pub fn successors(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Successor)
    }

    /// Iterates fields classified as [`FieldCategory::Region`].
    pub fn regions(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Region)
    }

    /// Iterates fields classified as [`FieldCategory::DiGraph`].
    pub fn digraphs(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::DiGraph)
    }

    /// Iterates fields classified as [`FieldCategory::UnGraph`].
    pub fn ungraphs(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::UnGraph)
    }

    /// Iterates fields classified as [`FieldCategory::Value`] (plain Rust types).
    pub fn values(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Value)
    }

    /// Returns the total number of fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns identifiers for all named fields (empty for tuple structs).
    pub fn named_field_idents(&self) -> Vec<syn::Ident> {
        self.fields.iter().filter_map(|f| f.ident.clone()).collect()
    }

    /// Returns `true` if fields are positional (tuple struct/variant).
    pub fn is_tuple_style(&self) -> bool {
        self.fields.iter().all(|f| f.ident.is_none())
    }

    /// Maps field names to their positional indices.
    pub fn field_name_to_index(&self) -> HashMap<String, usize> {
        self.fields
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|id| (id.to_string(), f.index)))
            .collect()
    }

    /// Builds [`FieldBindings`](crate::codegen::FieldBindings) with the given variable prefix.
    pub fn field_bindings(&self, prefix: &str) -> crate::codegen::FieldBindings {
        if self.is_tuple_style() {
            crate::codegen::FieldBindings::tuple(prefix, self.field_count())
        } else {
            crate::codegen::FieldBindings::named(prefix, self.named_field_idents())
        }
    }

    /// Clones all fields into a new `Vec`.
    pub fn collect_fields(&self) -> Vec<FieldInfo<L>> {
        self.fields.clone()
    }
}
