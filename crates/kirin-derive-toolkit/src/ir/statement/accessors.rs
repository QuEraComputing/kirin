use std::collections::HashMap;

use super::super::{fields::*, layout::Layout};
use super::definition::Statement;

impl<L: Layout> Statement<L> {
    pub fn iter_all_fields(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields.iter()
    }

    pub fn arguments(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Argument)
    }

    pub fn results(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Result)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Block)
    }

    pub fn successors(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Successor)
    }

    pub fn regions(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Region)
    }

    pub fn values(&self) -> impl Iterator<Item = &FieldInfo<L>> {
        self.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Value)
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    pub fn named_field_idents(&self) -> Vec<syn::Ident> {
        self.fields.iter().filter_map(|f| f.ident.clone()).collect()
    }

    pub fn is_tuple_style(&self) -> bool {
        self.fields.iter().all(|f| f.ident.is_none())
    }

    pub fn field_name_to_index(&self) -> HashMap<String, usize> {
        self.fields
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|id| (id.to_string(), f.index)))
            .collect()
    }

    pub fn field_bindings(&self, prefix: &str) -> crate::codegen::FieldBindings {
        if self.is_tuple_style() {
            crate::codegen::FieldBindings::tuple(prefix, self.field_count())
        } else {
            crate::codegen::FieldBindings::named(prefix, self.named_field_idents())
        }
    }

    pub fn collect_fields(&self) -> Vec<FieldInfo<L>> {
        self.fields.clone()
    }
}
