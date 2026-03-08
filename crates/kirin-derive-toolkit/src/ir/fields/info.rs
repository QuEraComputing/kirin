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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DefaultValue, StandardLayout};
    use proc_macro2::Span;

    fn make_argument_field(index: usize, name: Option<&str>) -> FieldInfo<StandardLayout> {
        FieldInfo {
            index,
            ident: name.map(|n| syn::Ident::new(n, Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Argument {
                ssa_type: syn::parse_quote!(Default::default()),
            },
        }
    }

    fn make_result_field(index: usize, name: &str) -> FieldInfo<StandardLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Result {
                ssa_type: syn::parse_quote!(MyType),
            },
        }
    }

    fn make_value_field(
        index: usize,
        name: &str,
        default: Option<DefaultValue>,
        into: bool,
    ) -> FieldInfo<StandardLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Value {
                ty: syn::parse_quote!(i64),
                default,
                into,
                extra: (),
            },
        }
    }

    #[test]
    fn category_argument() {
        let f = make_argument_field(0, Some("x"));
        assert_eq!(f.category(), FieldCategory::Argument);
    }

    #[test]
    fn category_result() {
        let f = make_result_field(0, "out");
        assert_eq!(f.category(), FieldCategory::Result);
    }

    #[test]
    fn category_block() {
        let f: FieldInfo<StandardLayout> = FieldInfo {
            index: 0,
            ident: Some(syn::Ident::new("blk", Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Block,
        };
        assert_eq!(f.category(), FieldCategory::Block);
    }

    #[test]
    fn kind_name_all_categories() {
        let cases: Vec<(FieldData<StandardLayout>, &str)> = vec![
            (
                FieldData::Argument {
                    ssa_type: syn::parse_quote!(()),
                },
                "argument",
            ),
            (
                FieldData::Result {
                    ssa_type: syn::parse_quote!(()),
                },
                "result",
            ),
            (FieldData::Block, "block"),
            (FieldData::Successor, "successor"),
            (FieldData::Region, "region"),
            (FieldData::Symbol, "symbol"),
            (
                FieldData::Value {
                    ty: syn::parse_quote!(i32),
                    default: None,
                    into: false,
                    extra: (),
                },
                "value",
            ),
        ];
        for (data, expected_kind) in cases {
            let f: FieldInfo<StandardLayout> = FieldInfo {
                index: 0,
                ident: None,
                collection: Collection::Single,
                data,
            };
            assert_eq!(f.kind_name(), expected_kind);
        }
    }

    #[test]
    fn name_ident_named_field() {
        let f = make_argument_field(0, Some("my_field"));
        let ident = f.name_ident(Span::call_site());
        assert_eq!(ident, "my_field");
    }

    #[test]
    fn name_ident_positional_field() {
        let f = make_argument_field(3, None);
        let ident = f.name_ident(Span::call_site());
        assert_eq!(ident, "field_3");
    }

    #[test]
    fn has_default_with_default() {
        let f = make_value_field(0, "x", Some(DefaultValue::Default), false);
        assert!(f.has_default());
    }

    #[test]
    fn has_default_without_default() {
        let f = make_value_field(0, "x", None, false);
        assert!(!f.has_default());
    }

    #[test]
    fn has_default_on_non_value_field() {
        let f = make_argument_field(0, Some("x"));
        assert!(!f.has_default());
    }

    #[test]
    fn default_value_some() {
        let f = make_value_field(0, "x", Some(DefaultValue::Default), false);
        assert!(f.default_value().is_some());
    }

    #[test]
    fn default_value_none() {
        let f = make_value_field(0, "x", None, false);
        assert!(f.default_value().is_none());
    }

    #[test]
    fn default_value_on_argument() {
        let f = make_argument_field(0, Some("x"));
        assert!(f.default_value().is_none());
    }

    #[test]
    fn ssa_type_on_argument() {
        let f = make_argument_field(0, Some("x"));
        assert!(f.ssa_type().is_some());
    }

    #[test]
    fn ssa_type_on_result() {
        let f = make_result_field(0, "out");
        assert!(f.ssa_type().is_some());
    }

    #[test]
    fn ssa_type_on_value() {
        let f = make_value_field(0, "x", None, false);
        assert!(f.ssa_type().is_none());
    }

    #[test]
    fn value_type_on_value() {
        let f = make_value_field(0, "x", None, false);
        assert!(f.value_type().is_some());
    }

    #[test]
    fn value_type_on_argument() {
        let f = make_argument_field(0, Some("x"));
        assert!(f.value_type().is_none());
    }

    #[test]
    fn has_into_true() {
        let f = make_value_field(0, "x", None, true);
        assert!(f.has_into());
    }

    #[test]
    fn has_into_false() {
        let f = make_value_field(0, "x", None, false);
        assert!(!f.has_into());
    }

    #[test]
    fn has_into_on_non_value() {
        let f = make_argument_field(0, Some("x"));
        assert!(!f.has_into());
    }

    #[test]
    fn extra_on_value() {
        let f = make_value_field(0, "x", None, false);
        assert!(f.extra().is_some());
    }

    #[test]
    fn extra_on_non_value() {
        let f = make_argument_field(0, Some("x"));
        assert!(f.extra().is_none());
    }

    #[test]
    fn display_named() {
        let f = make_argument_field(0, Some("my_field"));
        assert_eq!(format!("{}", f), "my_field");
    }

    #[test]
    fn display_positional() {
        let f = make_argument_field(5, None);
        assert_eq!(format!("{}", f), "field_5");
    }
}
