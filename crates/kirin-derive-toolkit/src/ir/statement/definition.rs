use super::super::{
    attrs::{KirinFieldOptions, StatementOptions},
    fields::*,
    layout::Layout,
};
use darling::{FromDeriveInput, FromField, FromVariant};

/// A single IR operation — either a struct body or one enum variant.
///
/// Each statement has a name, parsed `#[kirin(...)]` options, and a list of
/// classified [`FieldInfo`] entries. If the variant uses `#[wraps]`, the
/// [`wraps`](Self::wraps) field contains the delegation target.
///
/// # Field Access
///
/// ```ignore
/// for field in stmt.arguments() {
///     // SSAValue fields
/// }
/// for field in stmt.results() {
///     // ResultValue fields
/// }
/// for field in stmt.values() {
///     // Plain Rust-type fields
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Statement<L: Layout> {
    pub name: syn::Ident,
    pub attrs: StatementOptions,
    pub fields: Vec<FieldInfo<L>>,
    pub wraps: Option<Wrapper>,
    pub extra: L::StatementExtra,
    pub extra_attrs: L::ExtraStatementAttrs,
    pub raw_attrs: Vec<syn::Attribute>,
}

impl<L: Layout> Statement<L> {
    pub fn new(
        name: syn::Ident,
        attrs: StatementOptions,
        extra: L::StatementExtra,
        extra_attrs: L::ExtraStatementAttrs,
        raw_attrs: Vec<syn::Attribute>,
    ) -> Self {
        Self {
            name,
            attrs,
            fields: Vec::new(),
            wraps: None,
            extra,
            extra_attrs,
            raw_attrs,
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
        Statement::new(
            input.ident.clone(),
            attrs,
            extra,
            extra_attrs,
            input.attrs.clone(),
        )
        .update_fields(
            input.attrs.iter().any(|attr| attr.path().is_ident("wraps")),
            &data.fields,
        )
    }

    pub fn from_variant(wraps: bool, variant: &syn::Variant) -> darling::Result<Self> {
        let attrs = StatementOptions::from_variant(variant)?;
        let extra = L::StatementExtra::from_variant(variant)?;
        let extra_attrs = L::ExtraStatementAttrs::from_variant(variant)?;
        Statement::new(
            variant.ident.clone(),
            attrs,
            extra,
            extra_attrs,
            variant.attrs.clone(),
        )
        .update_fields(
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

        for (i, f) in fields.iter().enumerate() {
            errors.handle_in(|| {
                self.fields.push(Self::parse_field(i, f)?);
                Ok(())
            });
        }
        errors.finish()?;
        Ok(self)
    }

    fn parse_field(index: usize, f: &syn::Field) -> darling::Result<FieldInfo<L>> {
        let kirin_opts = KirinFieldOptions::from_field(f)?;
        let extra = L::ExtraFieldAttrs::from_field(f)?;
        let ident = f.ident.clone();
        let ty = &f.ty;

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

        if let Some(collection) = Collection::from_type(ty, "Block") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Block,
            });
        }

        if let Some(collection) = Collection::from_type(ty, "Successor") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Successor,
            });
        }

        if let Some(collection) = Collection::from_type(ty, "Region") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Region,
            });
        }

        if let Some(collection) = Collection::from_type(ty, "Symbol") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Symbol,
            });
        }

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
}
