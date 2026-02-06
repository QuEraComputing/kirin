use std::collections::HashMap;

use super::{
    attrs::{KirinFieldOptions, StatementOptions},
    fields::*,
    layout::Layout,
};
use darling::{FromDeriveInput, FromField, FromVariant};

#[derive(Debug, Clone)]
pub struct Statement<L: Layout> {
    pub name: syn::Ident,
    pub attrs: StatementOptions,
    /// All fields in declaration order.
    pub fields: Vec<FieldInfo<L>>,
    pub wraps: Option<Wrapper>,
    pub extra: L::StatementExtra,
    pub extra_attrs: L::ExtraStatementAttrs,
}

impl<L: Layout> Statement<L> {
    pub fn new(
        name: syn::Ident,
        attrs: StatementOptions,
        extra: L::StatementExtra,
        extra_attrs: L::ExtraStatementAttrs,
    ) -> Self {
        Self {
            name,
            attrs,
            fields: Vec::new(),
            wraps: None,
            extra,
            extra_attrs,
        }
    }

    pub fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let syn::Data::Struct(data) = &input.data else {
            return Err(
                darling::Error::custom("Kirin statements can only be derived for structs")
                    .with_span(input),
            );
        };
        let attrs = StatementOptions::from_derive_input(input)?;
        let extra = L::StatementExtra::from_derive_input(input)?;
        let extra_attrs = L::ExtraStatementAttrs::from_derive_input(input)?;
        Statement::new(input.ident.clone(), attrs, extra, extra_attrs).update_fields(
            input.attrs.iter().any(|attr| attr.path().is_ident("wraps")),
            &data.fields,
        )
    }

    pub fn from_variant(wraps: bool, variant: &syn::Variant) -> darling::Result<Self> {
        let attrs = StatementOptions::from_variant(variant)?;
        let extra = L::StatementExtra::from_variant(variant)?;
        let extra_attrs = L::ExtraStatementAttrs::from_variant(variant)?;
        Statement::new(variant.ident.clone(), attrs, extra, extra_attrs).update_fields(
            wraps
                || variant
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("wraps")),
            &variant.fields,
        )
    }

    fn update_fields(mut self, wraps: bool, fields: &syn::Fields) -> darling::Result<Self> {
        let mut errors = darling::Error::accumulator();

        // Handle wrapper variants
        if wraps
            || fields
                .iter()
                .any(|f| f.attrs.iter().any(|attr| attr.path().is_ident("wraps")))
        {
            if fields.len() == 1 {
                self.wraps = Some(Wrapper::new(0, fields.iter().next().unwrap()));
            } else {
                for (i, f) in fields.iter().enumerate() {
                    errors.handle_in(|| {
                        if f.attrs.iter().any(|attr| attr.path().is_ident("wraps")) {
                            self.wraps = Some(Wrapper::new(i, f));
                        } else {
                            self.fields.push(Self::parse_field(i, f)?);
                        }
                        Ok(())
                    });
                }
            }

            if self.wraps.is_none() {
                errors.push(
                    darling::Error::custom("No field marked with #[wraps] attribute")
                        .with_span(fields),
                );
            }
            errors.finish()?;
            return Ok(self);
        }

        // Parse all fields
        for (i, f) in fields.iter().enumerate() {
            errors.handle_in(|| {
                self.fields.push(Self::parse_field(i, f)?);
                Ok(())
            });
        }
        errors.finish()?;
        Ok(self)
    }

    /// Parse a single field into FieldInfo.
    fn parse_field(index: usize, f: &syn::Field) -> darling::Result<FieldInfo<L>> {
        let kirin_opts = KirinFieldOptions::from_field(f)?;
        let extra = L::ExtraFieldAttrs::from_field(f)?;
        let ident = f.ident.clone();
        let ty = &f.ty;

        // Check for SSAValue (Argument)
        if let Some(collection) = Collection::from_type(ty, "SSAValue") {
            let ssa_type = kirin_opts
                .ssa_ty
                .unwrap_or_else(|| syn::parse_quote! { Default::default() });
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Argument { ssa_type },
            });
        }

        // Check for ResultValue (Result)
        if let Some(collection) = Collection::from_type(ty, "ResultValue") {
            let ssa_type = kirin_opts
                .ssa_ty
                .unwrap_or_else(|| syn::parse_quote! { Default::default() });
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Result { ssa_type },
            });
        }

        // Check for Block
        if let Some(collection) = Collection::from_type(ty, "Block") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Block,
            });
        }

        // Check for Successor
        if let Some(collection) = Collection::from_type(ty, "Successor") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Successor,
            });
        }

        // Check for Region
        if let Some(collection) = Collection::from_type(ty, "Region") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Region,
            });
        }

        // Check for Symbol
        if let Some(collection) = Collection::from_type(ty, "Symbol") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Symbol,
            });
        }

        // Otherwise it's a compile-time Value
        Ok(FieldInfo {
            index,
            ident,
            collection: Collection::Single,
            data: FieldData::Value {
                ty: ty.clone(),
                default: kirin_opts.default,
                into: kirin_opts.into,
                extra,
            },
        })
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StandardLayout;
    use quote::ToTokens;

    /// Helper to parse a struct and return the Statement
    fn parse_statement(input: proc_macro2::TokenStream) -> Statement<StandardLayout> {
        let input: syn::DeriveInput = syn::parse2(input).expect("Failed to parse input");
        Statement::from_derive_input(&input).expect("Failed to create Statement")
    }

    /// Helper to parse a variant from an enum
    fn parse_variant(input: proc_macro2::TokenStream) -> Statement<StandardLayout> {
        let input: syn::DeriveInput = syn::parse2(input).expect("Failed to parse input");
        let syn::Data::Enum(data) = &input.data else {
            panic!("Expected enum");
        };
        Statement::from_variant(false, &data.variants[0]).expect("Failed to create Statement")
    }

    #[test]
    fn test_field_count() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                arg: SSAValue,
                #[kirin(type = "T")]
                res: ResultValue,
                block: Block,
                succ: Successor,
                region: Region,
                value: String,
            }
        });

        assert_eq!(stmt.field_count(), 6);
    }

    #[test]
    fn test_field_count_empty() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct EmptyStmt {}
        });

        assert_eq!(stmt.field_count(), 0);
    }

    #[test]
    fn test_iter_all_fields_categories() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                arg: SSAValue,
                #[kirin(type = "T")]
                res: ResultValue,
                block: Block,
                succ: Successor,
                region: Region,
                value: String,
            }
        });

        let fields: Vec<_> = stmt.iter_all_fields().collect();
        assert_eq!(fields.len(), 6);

        // Fields are now in declaration order
        assert_eq!(fields[0].category(), FieldCategory::Argument);
        assert_eq!(fields[1].category(), FieldCategory::Result);
        assert_eq!(fields[2].category(), FieldCategory::Block);
        assert_eq!(fields[3].category(), FieldCategory::Successor);
        assert_eq!(fields[4].category(), FieldCategory::Region);
        assert_eq!(fields[5].category(), FieldCategory::Value);
    }

    #[test]
    fn test_category_iterators() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                arg1: SSAValue,
                #[kirin(type = "T")]
                res1: ResultValue,
                #[kirin(type = "T")]
                arg2: SSAValue,
                value: String,
            }
        });

        assert_eq!(stmt.arguments().count(), 2);
        assert_eq!(stmt.results().count(), 1);
        assert_eq!(stmt.values().count(), 1);
        assert_eq!(stmt.blocks().count(), 0);
    }

    #[test]
    fn test_collect_fields_declaration_order() {
        // Fields declared in non-category order
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                res: ResultValue,      // index 0, Result
                value: String,         // index 1, Value
                #[kirin(type = "T")]
                arg: SSAValue,         // index 2, Argument
                block: Block,          // index 3, Block
            }
        });

        let fields = stmt.collect_fields();
        assert_eq!(fields.len(), 4);

        // collect_fields should return in declaration order (by index)
        assert_eq!(fields[0].index, 0);
        assert_eq!(fields[0].ident.as_ref().unwrap().to_string(), "res");
        assert_eq!(fields[1].index, 1);
        assert_eq!(fields[1].ident.as_ref().unwrap().to_string(), "value");
        assert_eq!(fields[2].index, 2);
        assert_eq!(fields[2].ident.as_ref().unwrap().to_string(), "arg");
        assert_eq!(fields[3].index, 3);
        assert_eq!(fields[3].ident.as_ref().unwrap().to_string(), "block");
    }

    #[test]
    fn test_named_field_idents_declaration_order() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                res: ResultValue,      // index 0
                value: String,         // index 1
                #[kirin(type = "T")]
                arg: SSAValue,         // index 2
            }
        });

        let idents = stmt.named_field_idents();
        assert_eq!(idents.len(), 3);

        // Should be in declaration order
        assert_eq!(idents[0].to_string(), "res");
        assert_eq!(idents[1].to_string(), "value");
        assert_eq!(idents[2].to_string(), "arg");
    }

    #[test]
    fn test_is_tuple_style_named() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct NamedStmt {
                #[kirin(type = "T")]
                arg: SSAValue,
            }
        });

        assert!(!stmt.is_tuple_style());
    }

    #[test]
    fn test_is_tuple_style_tuple() {
        let stmt = parse_variant(quote::quote! {
            #[kirin(type = MyLattice)]
            enum MyEnum {
                TupleVariant(#[kirin(type = "T")] SSAValue, String),
            }
        });

        assert!(stmt.is_tuple_style());
    }

    #[test]
    fn test_field_name_to_index() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                first: SSAValue,       // index 0
                second: String,        // index 1
                #[kirin(type = "T")]
                third: ResultValue,    // index 2
            }
        });

        let map = stmt.field_name_to_index();
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("first"), Some(&0));
        assert_eq!(map.get("second"), Some(&1));
        assert_eq!(map.get("third"), Some(&2));
    }

    #[test]
    fn test_wrapper_detection() {
        let stmt = parse_variant(quote::quote! {
            #[kirin(type = MyLattice)]
            enum MyEnum {
                #[wraps]
                WrapperVariant(InnerType),
            }
        });

        assert!(stmt.wraps.is_some());
        let wrapper = stmt.wraps.as_ref().unwrap();
        assert_eq!(wrapper.field.index, 0);
    }

    #[test]
    fn test_wrapper_with_extra_fields() {
        let stmt = parse_variant(quote::quote! {
            #[kirin(type = MyLattice)]
            enum MyEnum {
                MultiField(#[wraps] InnerType, String),
            }
        });

        assert!(stmt.wraps.is_some());
        // Extra field should be in values
        assert_eq!(stmt.values().count(), 1);
    }

    #[test]
    fn test_field_data_argument() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "CustomType")]
                arg: SSAValue,
            }
        });

        let fields: Vec<_> = stmt.iter_all_fields().collect();
        assert_eq!(fields.len(), 1);

        match &fields[0].data {
            FieldData::Argument { ssa_type } => {
                // ssa_type should be the parsed expression
                assert!(
                    ssa_type
                        .to_token_stream()
                        .to_string()
                        .contains("CustomType")
                );
            }
            _ => panic!("Expected Argument"),
        }
    }

    #[test]
    fn test_field_data_value_with_default() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(default)]
                value: String,
            }
        });

        let fields: Vec<_> = stmt.iter_all_fields().collect();
        assert_eq!(fields.len(), 1);

        match &fields[0].data {
            FieldData::Value { default, .. } => {
                assert!(default.is_some());
            }
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_field_data_value_with_into() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(into)]
                value: String,
            }
        });

        let fields: Vec<_> = stmt.iter_all_fields().collect();
        assert_eq!(fields.len(), 1);

        match &fields[0].data {
            FieldData::Value { into, .. } => {
                assert!(*into);
            }
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_collection_types() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                single: SSAValue,
                #[kirin(type = "T")]
                vec_args: Vec<SSAValue>,
                #[kirin(type = "T")]
                opt_arg: Option<SSAValue>,
            }
        });

        let fields = stmt.collect_fields();
        assert_eq!(fields.len(), 3);

        assert_eq!(fields[0].collection, Collection::Single);
        assert_eq!(fields[1].collection, Collection::Vec);
        assert_eq!(fields[2].collection, Collection::Option);
    }

    #[test]
    fn test_field_bindings_named() {
        let stmt = parse_statement(quote::quote! {
            #[kirin(type = MyLattice)]
            struct MyStmt {
                #[kirin(type = "T")]
                first: SSAValue,
                second: String,
            }
        });

        let bindings = stmt.field_bindings("f");
        assert!(!bindings.is_tuple);
        assert_eq!(bindings.field_count, 2);
        assert_eq!(bindings.field_idents.len(), 2);
        assert_eq!(bindings.original_field_names.len(), 2);
    }

    #[test]
    fn test_field_bindings_tuple() {
        let stmt = parse_variant(quote::quote! {
            #[kirin(type = MyLattice)]
            enum MyEnum {
                Tuple(#[kirin(type = "T")] SSAValue, String),
            }
        });

        let bindings = stmt.field_bindings("f");
        assert!(bindings.is_tuple);
        assert_eq!(bindings.field_count, 2);
        assert_eq!(bindings.field_idents.len(), 2);
    }
}
