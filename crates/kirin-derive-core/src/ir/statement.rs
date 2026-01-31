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

    /// Iterates over all fields in this statement, providing common field information.
    ///
    /// Fields are yielded in the order: arguments, results, blocks, successors, regions, values.
    /// This is useful when you need to process all fields uniformly.
    pub fn iter_all_fields(&self) -> impl Iterator<Item = FieldInfo<'_>> {
        let args = self.arguments.iter().map(|a| FieldInfo {
            field: &a.field,
            collection: &a.collection,
            category: FieldCategory::Argument,
        });
        let results = self.results.iter().map(|r| FieldInfo {
            field: &r.field,
            collection: &r.collection,
            category: FieldCategory::Result,
        });
        let blocks = self.blocks.iter().map(|b| FieldInfo {
            field: &b.field,
            collection: &b.collection,
            category: FieldCategory::Block,
        });
        let successors = self.successors.iter().map(|s| FieldInfo {
            field: &s.field,
            collection: &s.collection,
            category: FieldCategory::Successor,
        });
        let regions = self.regions.iter().map(|r| FieldInfo {
            field: &r.field,
            collection: &r.collection,
            category: FieldCategory::Region,
        });
        let values = self.values.iter().map(|v| FieldInfo {
            field: &v.field,
            collection: &Collection::Single,
            category: FieldCategory::Value,
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

    /// Collects all named field identifiers.
    ///
    /// Returns identifiers only for fields that have names (not tuple fields).
    pub fn named_field_idents(&self) -> Vec<syn::Ident> {
        self.iter_all_fields()
            .filter_map(|f| f.field.ident.clone())
            .collect()
    }

    /// Returns true if all fields are unnamed (tuple-style).
    pub fn is_tuple_style(&self) -> bool {
        self.iter_all_fields().all(|f| f.field.ident.is_none())
    }

    /// Builds a map from field name to field index.
    ///
    /// Only includes fields that have names.
    pub fn field_name_to_index(&self) -> HashMap<String, usize> {
        self.iter_all_fields()
            .filter_map(|f| {
                f.field
                    .ident
                    .as_ref()
                    .map(|id| (id.to_string(), f.field.index))
            })
            .collect()
    }
}
