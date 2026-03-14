use std::ops::{Deref, DerefMut};

use darling::FromDeriveInput;

use super::{
    attrs::GlobalOptions,
    fields::Wrapper,
    layout::{HasCratePath, Layout},
    statement::Statement,
};

/// Top-level parsed representation of a derive macro input.
///
/// Wraps a `syn::DeriveInput` with Kirin-specific attribute parsing and
/// field classification. Access the parsed statements via [`data`](Self::data).
///
/// # Parsing
///
/// ```ignore
/// let input = Input::<StandardLayout>::from_derive_input(&ast)?;
/// match &input.data {
///     Data::Struct(s) => { /* single statement */ }
///     Data::Enum(e) => { /* multiple variants */ }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Input<L: Layout> {
    /// Name of the derive input type.
    pub name: syn::Ident,
    /// Generic parameters from the input type.
    pub generics: syn::Generics,
    /// Normalized `#[kirin(...)]` options from the top-level type.
    pub attrs: GlobalOptions,
    /// Layout-specific extra global attributes.
    pub extra_attrs: L::ExtraGlobalAttrs,
    /// Parsed body: a single struct or enum with variants.
    pub data: Data<L>,
    /// Original unprocessed attributes from the input.
    pub raw_attrs: Vec<syn::Attribute>,
}

impl<L: Layout> Input<L> {
    /// Parse a `syn::DeriveInput` into a Kirin-typed `Input`.
    ///
    /// Supports structs and enums; unions produce an error.
    pub fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let attrs = GlobalOptions::from_derive_input(input)?;
        let extra_attrs = L::ExtraGlobalAttrs::from_derive_input(input)?;
        let ir_type = &attrs.ir_type;

        let data = match &input.data {
            syn::Data::Struct(_) => {
                Data::Struct(DataStruct(Statement::from_derive_input(input, ir_type)?))
            }
            syn::Data::Enum(data) => {
                let has_hidden_variants = data
                    .variants
                    .iter()
                    .any(|v| v.ident.to_string().starts_with("__"));
                let variants = data
                    .variants
                    .iter()
                    .filter(|v| !v.ident.to_string().starts_with("__"))
                    .map(|v| {
                        Statement::from_variant(
                            input.attrs.iter().any(|f| f.path().is_ident("wraps")),
                            v,
                            ir_type,
                        )
                    })
                    .collect::<darling::Result<Vec<_>>>()?;
                Data::Enum(DataEnum {
                    variants,
                    has_hidden_variants,
                })
            }
            syn::Data::Union(_) => {
                return Err(darling::Error::custom(
                    "Kirin ASTs can only be derived for structs or enums",
                )
                .with_span(input));
            }
        };

        Ok(Self {
            name: input.ident.clone(),
            generics: input.generics.clone(),
            attrs,
            extra_attrs,
            data,
            raw_attrs: input.attrs.clone(),
        })
    }
}

impl<L: Layout> Input<L>
where
    L::ExtraGlobalAttrs: HasCratePath,
{
    /// Resolve the downstream derive's crate path with a default.
    pub fn extra_crate_path(&self, default: fn() -> syn::Path) -> syn::Path {
        self.extra_attrs
            .crate_path()
            .cloned()
            .unwrap_or_else(default)
    }
}

/// The body of the derive input — either a single struct or an enum with variants.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Data<L: Layout> {
    Struct(DataStruct<L>),
    Enum(DataEnum<L>),
}

/// A struct-style input, containing a single [`Statement`].
///
/// Derefs to the inner `Statement<L>` for convenience.
#[derive(Debug, Clone)]
pub struct DataStruct<L: Layout>(pub Statement<L>);

impl<L: Layout> Deref for DataStruct<L> {
    type Target = Statement<L>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<L: Layout> DerefMut for DataStruct<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// An enum-style input, containing one [`Statement`] per variant.
///
/// Use [`iter_variants`](Self::iter_variants) for iteration that distinguishes
/// wrapper variants (marked with `#[wraps]`) from regular ones.
#[derive(Debug, Clone)]
pub struct DataEnum<L: Layout> {
    /// One [`Statement`] per enum variant (excludes hidden `__`-prefixed variants).
    pub variants: Vec<Statement<L>>,
    /// Whether the original enum has hidden variants (e.g. `__Phantom`) that
    /// were filtered out. When true, generated match expressions include a
    /// `_ => unreachable!()` wildcard arm.
    pub has_hidden_variants: bool,
}

impl<L: Layout> DataEnum<L> {
    /// Iterate variants, distinguishing `#[wraps]` wrappers from regular variants.
    pub fn iter_variants(&self) -> impl Iterator<Item = VariantRef<'_, L>> {
        self.variants.iter().map(|stmt| {
            if let Some(wrapper) = &stmt.wraps {
                VariantRef::Wrapper {
                    name: &stmt.name,
                    wrapper,
                    stmt,
                }
            } else {
                VariantRef::Regular {
                    name: &stmt.name,
                    stmt,
                }
            }
        })
    }
}

impl<L: Layout> Deref for DataEnum<L> {
    type Target = [Statement<L>];

    fn deref(&self) -> &Self::Target {
        &self.variants
    }
}

impl<L: Layout> DerefMut for DataEnum<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.variants
    }
}

/// Reference to an enum variant, distinguishing wrappers from regular variants.
///
/// Wrapper variants delegate to an inner type via `#[wraps]`; regular variants
/// have their own fields.
#[derive(Debug, Clone, Copy)]
pub enum VariantRef<'a, L: Layout> {
    Wrapper {
        name: &'a syn::Ident,
        wrapper: &'a Wrapper,
        stmt: &'a Statement<L>,
    },
    Regular {
        name: &'a syn::Ident,
        stmt: &'a Statement<L>,
    },
}

impl<'a, L: Layout> VariantRef<'a, L> {
    /// Return the variant's identifier.
    pub fn name(&self) -> &'a syn::Ident {
        match self {
            VariantRef::Wrapper { name, .. } => name,
            VariantRef::Regular { name, .. } => name,
        }
    }

    /// Return the underlying [`Statement`].
    pub fn stmt(&self) -> &'a Statement<L> {
        match self {
            VariantRef::Wrapper { stmt, .. } => stmt,
            VariantRef::Regular { stmt, .. } => stmt,
        }
    }

    /// Return `true` if this variant delegates via `#[wraps]`.
    pub fn is_wrapper(&self) -> bool {
        matches!(self, VariantRef::Wrapper { .. })
    }
}
