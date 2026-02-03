use std::collections::HashMap;

use super::{attrs::StatementOptions, fields::*, layout::Layout};
use darling::{FromDeriveInput, FromVariant};

#[derive(Debug, Clone)]
pub struct Statement<L: Layout> {
    pub name: syn::Ident,
    pub attrs: StatementOptions,
    pub arguments: Arguments,
    pub results: Results,
    pub blocks: Blocks,
    pub successors: Successors,
    pub regions: Regions,
    /// compile-time values of the statement
    pub values: CompileTimeValues<L>,
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
            arguments: Default::default(),
            results: Default::default(),
            blocks: Default::default(),
            successors: Default::default(),
            regions: Default::default(),
            values: Default::default(),
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
                            Ok(())
                        } else {
                            self.values.add(i, f)?;
                            Ok(())
                        }
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

        for (i, f) in fields.iter().enumerate() {
            errors.handle_in(|| {
                if self.arguments.add(i, f)?
                    || self.results.add(i, f)?
                    || self.blocks.add(i, f)?
                    || self.successors.add(i, f)?
                    || self.regions.add(i, f)?
                    || self.values.add(i, f)?
                {
                    return Ok(true);
                }
                Ok(true)
            });
        }
        errors.finish()?;
        Ok(self)
    }

    /// Iterates over all fields in this statement, returning owned field information.
    ///
    /// Fields are yielded in the order: arguments, results, blocks, successors, regions, values.
    /// This is useful when you need to process all fields uniformly.
    pub fn iter_all_fields(&self) -> impl Iterator<Item = FieldInfo<L>> + '_ {
        let args = self.arguments.iter().map(|a| FieldInfo {
            index: a.field.index,
            ident: a.field.ident.clone(),
            collection: a.collection.clone(),
            data: FieldData::Argument {
                ssa_type: a.ty.clone(),
            },
        });
        let results = self.results.iter().map(|r| FieldInfo {
            index: r.field.index,
            ident: r.field.ident.clone(),
            collection: r.collection.clone(),
            data: FieldData::Result {
                ssa_type: r.ty.clone(),
            },
        });
        let blocks = self.blocks.iter().map(|b| FieldInfo {
            index: b.field.index,
            ident: b.field.ident.clone(),
            collection: b.collection.clone(),
            data: FieldData::Block,
        });
        let successors = self.successors.iter().map(|s| FieldInfo {
            index: s.field.index,
            ident: s.field.ident.clone(),
            collection: s.collection.clone(),
            data: FieldData::Successor,
        });
        let regions = self.regions.iter().map(|r| FieldInfo {
            index: r.field.index,
            ident: r.field.ident.clone(),
            collection: r.collection.clone(),
            data: FieldData::Region,
        });
        let values = self.values.iter().map(|v| FieldInfo {
            index: v.field.index,
            ident: v.field.ident.clone(),
            collection: Collection::Single,
            data: FieldData::Value {
                ty: v.ty.clone(),
                default: v.default.clone(),
                into: v.into,
                extra: v.extra.clone(),
            },
        });

        args.chain(results)
            .chain(blocks)
            .chain(successors)
            .chain(regions)
            .chain(values)
    }

    /// Returns the total count of fields across all categories.
    pub fn field_count(&self) -> usize {
        self.arguments.iter().count()
            + self.results.iter().count()
            + self.blocks.iter().count()
            + self.successors.iter().count()
            + self.regions.iter().count()
            + self.values.iter().count()
    }

    /// Collects all named field identifiers in declaration order (sorted by index).
    ///
    /// Returns identifiers only for fields that have names (not tuple fields).
    pub fn named_field_idents(&self) -> Vec<syn::Ident> {
        let mut fields: Vec<_> = self.iter_all_fields().collect();
        fields.sort_by_key(|f| f.index);
        fields.into_iter().filter_map(|f| f.ident).collect()
    }

    /// Returns true if all fields are unnamed (tuple-style).
    pub fn is_tuple_style(&self) -> bool {
        self.iter_all_fields().all(|f| f.ident.is_none())
    }

    /// Builds a map from field name to field index.
    ///
    /// Only includes fields that have names.
    pub fn field_name_to_index(&self) -> HashMap<String, usize> {
        self.iter_all_fields()
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

    /// Collects all fields into a Vec, sorted by declaration order (index).
    ///
    /// This is a convenience method equivalent to `iter_all_fields().collect()`
    /// followed by sorting.
    pub fn collect_fields(&self) -> Vec<FieldInfo<L>> {
        let mut fields: Vec<_> = self.iter_all_fields().collect();
        fields.sort_by_key(|f| f.index);
        fields
    }
}
