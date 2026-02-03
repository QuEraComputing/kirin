mod collection;
mod comptime;
mod index;
mod value;
mod wrapper;

pub use collection::Collection;
pub use comptime::{CompileTimeValue, CompileTimeValues};
pub use index::FieldIndex;
pub use value::{Argument, Arguments, Result, Results, Value};
pub use wrapper::Wrapper;

use crate::ir::{DefaultValue, Layout};
use proc_macro2::Span;

/// Macro to define a simple IR field collection type.
///
/// This generates:
/// - A container struct (e.g., `Blocks`) with `data: Vec<T>`
/// - An item struct (e.g., `Block`) with `field: FieldIndex` and `collection: Collection`
/// - `add()` method that checks for the type name in field type
/// - `iter()` method to iterate over items
macro_rules! define_field_collection {
    ($container:ident, $item:ident, $type_name:literal) => {
        #[derive(Debug, Clone, Default)]
        pub struct $container {
            data: Vec<$item>,
        }

        #[derive(Debug, Clone)]
        pub struct $item {
            pub field: FieldIndex,
            pub collection: Collection,
        }

        impl $container {
            pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
                let Some(coll) = Collection::from_type(&f.ty, $type_name) else {
                    return Ok(false);
                };
                self.data.push($item {
                    field: FieldIndex::new(f.ident.clone(), index),
                    collection: coll,
                });
                Ok(true)
            }

            pub fn iter(&self) -> impl Iterator<Item = &$item> {
                self.data.iter()
            }
        }
    };
}

define_field_collection!(Blocks, Block, "Block");
define_field_collection!(Regions, Region, "Region");
define_field_collection!(Successors, Successor, "Successor");

/// The category of an IR field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldCategory {
    /// SSAValue input field
    Argument,
    /// ResultValue output field
    Result,
    /// Block field (owned control flow block)
    Block,
    /// Successor field (branch target)
    Successor,
    /// Region field (nested scope)
    Region,
    /// Compile-time value field
    Value,
}

/// Category-specific field data (owned).
///
/// This enum stores the data that varies by field category:
/// - `Argument` and `Result`: SSA type expression
/// - `Value`: type, default, into flag, and layout-specific extra data
/// - `Block`, `Successor`, `Region`: no additional data
#[derive(Debug, Clone)]
pub enum FieldData<L: Layout> {
    /// SSAValue argument field
    Argument {
        /// The SSA type expression from `#[kirin(type = ...)]`
        ssa_type: syn::Expr,
    },
    /// ResultValue output field
    Result {
        /// The SSA type expression from `#[kirin(type = ...)]`
        ssa_type: syn::Expr,
    },
    /// Block field (owned control flow block)
    Block,
    /// Successor field (branch target)
    Successor,
    /// Region field (nested scope)
    Region,
    /// Compile-time value field
    Value {
        /// The type of the compile-time value
        ty: syn::Type,
        /// Default value if specified via `#[kirin(default)]` or `#[kirin(default = ...)]`
        default: Option<DefaultValue>,
        /// Whether the `#[kirin(into)]` attribute is specified
        into: bool,
        /// Layout-specific extra data from field attributes
        extra: L::ExtraFieldAttrs,
    },
}

/// Unified field information for iteration and storage.
///
/// This struct provides a common representation for all field types,
/// used for both iteration over statement fields and storage in
/// data structures like `StatementInfo`.
#[derive(Debug, Clone)]
pub struct FieldInfo<L: Layout> {
    /// The positional index of this field in the struct/variant.
    pub index: usize,
    /// The field identifier (None for tuple fields).
    pub ident: Option<syn::Ident>,
    /// The collection type (Single, Vec, Option).
    pub collection: Collection,
    /// Category-specific data.
    pub data: FieldData<L>,
}

impl<L: Layout> FieldInfo<L> {
    /// Returns the category of this field (derived from the data variant).
    pub fn category(&self) -> FieldCategory {
        match &self.data {
            FieldData::Argument { .. } => FieldCategory::Argument,
            FieldData::Result { .. } => FieldCategory::Result,
            FieldData::Block => FieldCategory::Block,
            FieldData::Successor => FieldCategory::Successor,
            FieldData::Region => FieldCategory::Region,
            FieldData::Value { .. } => FieldCategory::Value,
        }
    }

    /// Returns a human-readable name for this field kind.
    pub fn kind_name(&self) -> &'static str {
        match self.category() {
            FieldCategory::Argument => "ssa_value",
            FieldCategory::Result => "result_value",
            FieldCategory::Block => "block",
            FieldCategory::Successor => "successor",
            FieldCategory::Region => "region",
            FieldCategory::Value => "value",
        }
    }

    /// Returns the name identifier for this field, with a fallback for tuple fields.
    pub fn name_ident(&self, fallback_span: Span) -> syn::Ident {
        self.ident
            .clone()
            .unwrap_or_else(|| syn::Ident::new(&format!("field_{}", self.index), fallback_span))
    }

    /// Returns true if this field has a default value.
    pub fn has_default(&self) -> bool {
        matches!(&self.data, FieldData::Value { default: Some(_), .. })
    }

    /// Returns the default value for Value fields, if any.
    pub fn default_value(&self) -> Option<&DefaultValue> {
        match &self.data {
            FieldData::Value { default, .. } => default.as_ref(),
            _ => None,
        }
    }

    /// Returns the SSA type expression for Argument or Result fields.
    pub fn ssa_type(&self) -> Option<&syn::Expr> {
        match &self.data {
            FieldData::Argument { ssa_type } | FieldData::Result { ssa_type } => Some(ssa_type),
            _ => None,
        }
    }

    /// Returns the value type for Value fields.
    pub fn value_type(&self) -> Option<&syn::Type> {
        match &self.data {
            FieldData::Value { ty, .. } => Some(ty),
            _ => None,
        }
    }

    /// Returns true if this Value field has the `into` attribute.
    pub fn has_into(&self) -> bool {
        matches!(&self.data, FieldData::Value { into: true, .. })
    }

    /// Returns the extra field attributes for Value fields.
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
