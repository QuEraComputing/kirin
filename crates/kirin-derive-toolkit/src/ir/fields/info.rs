use crate::ir::{DefaultValue, Layout};
use proc_macro2::Span;

use super::{Collection, FieldCategory, FieldData};

/// Complete metadata about a single field in a [`Statement`](crate::ir::Statement).
///
/// Combines positional info (`index`, `ident`), collection wrapping, and
/// category-specific data. Use [`category()`](Self::category) to branch on
/// the field's role.
///
/// ```ignore
/// match field.category() {
///     FieldCategory::Argument => { /* field.ssa_type() is Some */ }
///     FieldCategory::Value => { /* field.value_type() is Some */ }
///     _ => {}
/// }
/// ```
#[derive(Debug)]
pub struct FieldInfo<L: Layout> {
    /// Zero-based position within the struct/variant fields.
    pub index: usize,
    /// Field name, or `None` for tuple fields.
    pub ident: Option<syn::Ident>,
    /// Collection wrapping (`Single`, `Vec`, or `Option`).
    pub collection: Collection,
    /// Category-specific semantic data.
    pub data: FieldData<L>,
}

impl<L: Layout> Clone for FieldInfo<L> {
    fn clone(&self) -> Self {
        FieldInfo {
            index: self.index,
            ident: self.ident.clone(),
            collection: self.collection.clone(),
            data: self.data.clone(),
        }
    }
}

impl<L: Layout> FieldInfo<L> {
    /// Return the semantic category of this field.
    pub fn category(&self) -> FieldCategory {
        match &self.data {
            FieldData::Argument { .. } => FieldCategory::Argument,
            FieldData::Result { .. } => FieldCategory::Result,
            FieldData::Block => FieldCategory::Block,
            FieldData::Successor => FieldCategory::Successor,
            FieldData::Region => FieldCategory::Region,
            FieldData::Symbol => FieldCategory::Symbol,
            FieldData::Value { .. } => FieldCategory::Value,
        }
    }

    /// Return the category as a lowercase string (e.g. `"argument"`, `"value"`).
    pub fn kind_name(&self) -> &'static str {
        match self.category() {
            FieldCategory::Argument => "argument",
            FieldCategory::Result => "result",
            FieldCategory::Block => "block",
            FieldCategory::Successor => "successor",
            FieldCategory::Region => "region",
            FieldCategory::Symbol => "symbol",
            FieldCategory::Value => "value",
        }
    }

    /// Return the field's identifier, or synthesize `field_{index}` for tuple fields.
    pub fn name_ident(&self, fallback_span: Span) -> syn::Ident {
        self.ident
            .clone()
            .unwrap_or_else(|| syn::Ident::new(&format!("field_{}", self.index), fallback_span))
    }

    /// Return `true` if this `Value` field has a `#[kirin(default)]` annotation.
    pub fn has_default(&self) -> bool {
        matches!(
            &self.data,
            FieldData::Value {
                default: Some(_),
                ..
            }
        )
    }

    /// Return the default value specification, if any.
    pub fn default_value(&self) -> Option<&DefaultValue> {
        match &self.data {
            FieldData::Value { default, .. } => default.as_ref(),
            _ => None,
        }
    }

    /// Return the SSA type expression for `Argument` or `Result` fields.
    pub fn ssa_type(&self) -> Option<&syn::Expr> {
        match &self.data {
            FieldData::Argument { ssa_type } | FieldData::Result { ssa_type } => Some(ssa_type),
            _ => None,
        }
    }

    /// Return the Rust type for `Value` fields.
    pub fn value_type(&self) -> Option<&syn::Type> {
        match &self.data {
            FieldData::Value { ty, .. } => Some(ty),
            _ => None,
        }
    }

    /// Return `true` if this `Value` field has `#[kirin(into)]`, enabling `.into()` coercion.
    pub fn has_into(&self) -> bool {
        matches!(&self.data, FieldData::Value { into: true, .. })
    }

    /// Return layout-specific extra attributes for `Value` fields.
    pub fn extra(&self) -> Option<&L::ExtraFieldAttrs> {
        match &self.data {
            FieldData::Value { extra, .. } => Some(extra),
            _ => None,
        }
    }
}

impl<L: Layout> std::fmt::Display for FieldInfo<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.ident {
            Some(ident) => write!(f, "{}", ident),
            None => write!(f, "field_{}", self.index),
        }
    }
}
