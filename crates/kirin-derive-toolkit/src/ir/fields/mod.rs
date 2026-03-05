mod collection;
mod index;
mod wrapper;

pub use collection::Collection;
pub use index::FieldIndex;
pub use wrapper::Wrapper;

use crate::ir::{DefaultValue, Layout};
use proc_macro2::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldCategory {
    Argument,
    Result,
    Block,
    Successor,
    Region,
    Symbol,
    Value,
}

#[derive(Debug)]
pub enum FieldData<L: Layout> {
    Argument { ssa_type: syn::Expr },
    Result { ssa_type: syn::Expr },
    Block,
    Successor,
    Region,
    Symbol,
    Value {
        ty: syn::Type,
        default: Option<DefaultValue>,
        into: bool,
        extra: L::ExtraFieldAttrs,
    },
}

impl<L: Layout> Clone for FieldData<L> {
    fn clone(&self) -> Self {
        match self {
            FieldData::Argument { ssa_type } => FieldData::Argument {
                ssa_type: ssa_type.clone(),
            },
            FieldData::Result { ssa_type } => FieldData::Result {
                ssa_type: ssa_type.clone(),
            },
            FieldData::Block => FieldData::Block,
            FieldData::Successor => FieldData::Successor,
            FieldData::Region => FieldData::Region,
            FieldData::Symbol => FieldData::Symbol,
            FieldData::Value {
                ty,
                default,
                into,
                extra,
            } => FieldData::Value {
                ty: ty.clone(),
                default: default.clone(),
                into: *into,
                extra: extra.clone(),
            },
        }
    }
}

#[derive(Debug)]
pub struct FieldInfo<L: Layout> {
    pub index: usize,
    pub ident: Option<syn::Ident>,
    pub collection: Collection,
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

    pub fn name_ident(&self, fallback_span: Span) -> syn::Ident {
        self.ident
            .clone()
            .unwrap_or_else(|| syn::Ident::new(&format!("field_{}", self.index), fallback_span))
    }

    pub fn has_default(&self) -> bool {
        matches!(
            &self.data,
            FieldData::Value {
                default: Some(_),
                ..
            }
        )
    }

    pub fn default_value(&self) -> Option<&DefaultValue> {
        match &self.data {
            FieldData::Value { default, .. } => default.as_ref(),
            _ => None,
        }
    }

    pub fn ssa_type(&self) -> Option<&syn::Expr> {
        match &self.data {
            FieldData::Argument { ssa_type } | FieldData::Result { ssa_type } => Some(ssa_type),
            _ => None,
        }
    }

    pub fn value_type(&self) -> Option<&syn::Type> {
        match &self.data {
            FieldData::Value { ty, .. } => Some(ty),
            _ => None,
        }
    }

    pub fn has_into(&self) -> bool {
        matches!(&self.data, FieldData::Value { into: true, .. })
    }

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
