//! Field kind enumeration for code generation.
//!
//! This module provides a unified `FieldKind` type used by both AST generation
//! and parser generation.

use kirin_derive_core::ir::fields::{Collection, FieldCategory, FieldIndex};

use crate::ChumskyLayout;

/// The kind of a field in code generation context.
///
/// This extends `FieldCategory` with the actual type information for value fields.
#[derive(Debug, Clone)]
pub enum FieldKind {
    /// SSAValue input field
    SSAValue,
    /// ResultValue output field
    ResultValue,
    /// Block field (owned control flow block)
    Block,
    /// Successor field (branch target)
    Successor,
    /// Region field (nested scope)
    Region,
    /// Compile-time value field with its type
    Value(syn::Type),
}

impl FieldKind {
    /// Returns a human-readable name for this field kind.
    pub fn name(&self) -> &'static str {
        match self {
            FieldKind::SSAValue => "ssa_value",
            FieldKind::ResultValue => "result_value",
            FieldKind::Block => "block",
            FieldKind::Successor => "successor",
            FieldKind::Region => "region",
            FieldKind::Value(_) => "value",
        }
    }
}

/// Collected field information used during code generation.
///
/// This combines the field index, identifier, collection type, and kind
/// into a single structure for processing.
#[derive(Debug, Clone)]
pub struct CollectedField {
    /// The positional index of this field
    pub index: usize,
    /// The field identifier (None for tuple fields)
    pub ident: Option<syn::Ident>,
    /// The collection type (Single, Vec, Option)
    pub collection: Collection,
    /// The kind of this field
    pub kind: FieldKind,
}

impl std::fmt::Display for CollectedField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.ident {
            Some(ident) => write!(f, "{}", ident),
            None => write!(f, "field_{}", self.index),
        }
    }
}

/// Collects all fields from a statement into a sorted vector.
///
/// Fields are sorted by their positional index to ensure consistent ordering.
pub fn collect_fields(
    stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
) -> Vec<CollectedField> {
    let mut fields = Vec::new();

    for arg in stmt.arguments.iter() {
        fields.push(CollectedField {
            index: arg.field.index,
            ident: arg.field.ident.clone(),
            collection: arg.collection.clone(),
            kind: FieldKind::SSAValue,
        });
    }

    for res in stmt.results.iter() {
        fields.push(CollectedField {
            index: res.field.index,
            ident: res.field.ident.clone(),
            collection: res.collection.clone(),
            kind: FieldKind::ResultValue,
        });
    }

    for block in stmt.blocks.iter() {
        fields.push(CollectedField {
            index: block.field.index,
            ident: block.field.ident.clone(),
            collection: block.collection.clone(),
            kind: FieldKind::Block,
        });
    }

    for succ in stmt.successors.iter() {
        fields.push(CollectedField {
            index: succ.field.index,
            ident: succ.field.ident.clone(),
            collection: succ.collection.clone(),
            kind: FieldKind::Successor,
        });
    }

    for region in stmt.regions.iter() {
        fields.push(CollectedField {
            index: region.field.index,
            ident: region.field.ident.clone(),
            collection: region.collection.clone(),
            kind: FieldKind::Region,
        });
    }

    for value in stmt.values.iter() {
        fields.push(CollectedField {
            index: value.field.index,
            ident: value.field.ident.clone(),
            collection: Collection::Single,
            kind: FieldKind::Value(value.ty.clone()),
        });
    }

    fields.sort_by_key(|f| f.index);
    fields
}
