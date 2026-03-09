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
    /// The struct or variant identifier.
    pub name: syn::Ident,
    /// Parsed `#[kirin(...)]` options for this statement.
    pub attrs: StatementOptions,
    /// Classified fields (arguments, results, values, etc.).
    pub fields: Vec<FieldInfo<L>>,
    /// Delegation target if this variant uses `#[wraps]`.
    pub wraps: Option<Wrapper>,
    /// Layout-specific extra data computed per statement.
    pub extra: L::StatementExtra,
    /// Layout-specific extra attributes parsed from the variant.
    pub extra_attrs: L::ExtraStatementAttrs,
    /// Original unprocessed attributes.
    pub raw_attrs: Vec<syn::Attribute>,
}

impl<L: Layout> Statement<L> {
    /// Create a statement with the given metadata and no fields.
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

    /// Parse a struct-shaped `DeriveInput` into a `Statement`, classifying all fields.
    pub fn from_derive_input(
        input: &syn::DeriveInput,
        ir_type: &syn::Path,
    ) -> darling::Result<Self> {
        let syn::Data::Struct(data) = &input.data else {
            return Err(
                darling::Error::custom("Kirin statements can only be derived for structs")
                    .with_span(input),
            );
        };
        let attrs = StatementOptions::from_derive_input(input)?;
        let extra = L::StatementExtra::from_derive_input(input)?;
        let extra_attrs = L::extra_statement_attrs_from_input(input)?;
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
            ir_type,
        )
    }

    /// Parse an enum variant into a `Statement`, classifying all fields.
    ///
    /// `wraps` indicates whether the parent enum has a top-level `#[wraps]` attribute.
    pub fn from_variant(
        wraps: bool,
        variant: &syn::Variant,
        ir_type: &syn::Path,
    ) -> darling::Result<Self> {
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
            ir_type,
        )
    }

    fn update_fields(
        mut self,
        wraps: bool,
        fields: &syn::Fields,
        ir_type: &syn::Path,
    ) -> darling::Result<Self> {
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
                            self.fields.push(Self::parse_field(i, f, ir_type)?);
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
                self.fields.push(Self::parse_field(i, f, ir_type)?);
                Ok(())
            });
        }
        errors.finish()?;
        Ok(self)
    }

    fn parse_field(
        index: usize,
        f: &syn::Field,
        ir_type: &syn::Path,
    ) -> darling::Result<FieldInfo<L>> {
        let kirin_opts = KirinFieldOptions::from_field(f)?;
        let extra = L::ExtraFieldAttrs::from_field(f)?;
        let ident = f.ident.clone();
        let ty = &f.ty;

        if let Some(collection) = Collection::from_type(ty, "SSAValue") {
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Argument {
                    ssa_type: kirin_opts
                        .ssa_ty
                        .unwrap_or_else(|| syn::parse_quote! { () }),
                },
            });
        }

        if let Some(collection) = Collection::from_type(ty, "ResultValue") {
            let (ssa_type, is_auto_placeholder) = match kirin_opts.ssa_ty {
                Some(expr) => (expr, false),
                None => (syn::parse_quote!(#ir_type::placeholder()), true),
            };
            return Ok(FieldInfo {
                index,
                ident,
                collection,
                data: FieldData::Result {
                    ssa_type,
                    is_auto_placeholder,
                },
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
