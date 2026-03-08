use crate::ir::{DefaultValue, Layout};

/// Classification of a field's semantic role in an IR statement.
///
/// Determined automatically from the field's Rust type during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldCategory {
    /// SSA input value (`SSAValue` / `SSAValue<T>`).
    Argument,
    /// SSA output value (`ResultValue` / `ResultValue<T>`).
    Result,
    /// Basic block reference (`Block`).
    Block,
    /// Control-flow successor (`Successor`).
    Successor,
    /// Nested region (`Region` / `Region<T>`).
    Region,
    /// Symbol reference (`Symbol`).
    Symbol,
    /// Plain Rust value (anything not recognized as an IR primitive).
    Value,
}

impl FieldCategory {
    /// Returns true for categories that represent SSA values (Argument, Result).
    pub fn is_ssa_like(&self) -> bool {
        matches!(self, FieldCategory::Argument | FieldCategory::Result)
    }
}

/// Semantic data associated with a field, varying by [`FieldCategory`].
///
/// `Argument` and `Result` variants carry an `ssa_type` expression.
/// `Value` carries the original Rust type and optional default/into metadata.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum FieldData<L: Layout> {
    Argument {
        ssa_type: syn::Expr,
    },
    Result {
        ssa_type: syn::Expr,
    },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_category_is_ssa_like_argument() {
        assert!(FieldCategory::Argument.is_ssa_like());
    }

    #[test]
    fn field_category_is_ssa_like_result() {
        assert!(FieldCategory::Result.is_ssa_like());
    }

    #[test]
    fn field_category_is_ssa_like_block() {
        assert!(!FieldCategory::Block.is_ssa_like());
    }

    #[test]
    fn field_category_is_ssa_like_successor() {
        assert!(!FieldCategory::Successor.is_ssa_like());
    }

    #[test]
    fn field_category_is_ssa_like_region() {
        assert!(!FieldCategory::Region.is_ssa_like());
    }

    #[test]
    fn field_category_is_ssa_like_symbol() {
        assert!(!FieldCategory::Symbol.is_ssa_like());
    }

    #[test]
    fn field_category_is_ssa_like_value() {
        assert!(!FieldCategory::Value.is_ssa_like());
    }
}
