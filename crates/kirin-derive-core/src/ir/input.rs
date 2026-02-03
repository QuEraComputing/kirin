use std::ops::{Deref, DerefMut};

use darling::FromDeriveInput;

use super::{attrs::GlobalOptions, fields::Wrapper, layout::Layout, statement::Statement};

#[derive(Debug, Clone)]
pub struct Input<L: Layout> {
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub attrs: GlobalOptions,
    pub extra_attrs: L::ExtraGlobalAttrs,
    pub data: Data<L>,
}

impl<L: Layout> Input<L> {
    pub fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        match &input.data {
            syn::Data::Struct(_) => Ok(Self {
                name: input.ident.clone(),
                generics: input.generics.clone(),
                attrs: GlobalOptions::from_derive_input(input)?,
                extra_attrs: L::ExtraGlobalAttrs::from_derive_input(input)?,
                data: Data::Struct(DataStruct(Statement::from_derive_input(input)?)),
            }),
            syn::Data::Enum(data) => Ok(Self {
                name: input.ident.clone(),
                generics: input.generics.clone(),
                attrs: GlobalOptions::from_derive_input(input)?,
                extra_attrs: L::ExtraGlobalAttrs::from_derive_input(input)?,
                data: Data::Enum(DataEnum {
                    variants: data
                        .variants
                        .iter()
                        .map(|v| {
                            Statement::from_variant(
                                input.attrs.iter().any(|f| f.path().is_ident("wraps")),
                                v,
                            )
                        })
                        .collect::<darling::Result<Vec<_>>>()?,
                }),
            }),
            syn::Data::Union(_) => Err(darling::Error::custom(
                "Kirin ASTs can only be derived for structs or enums",
            )
            .with_span(input)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data<L: Layout> {
    Struct(DataStruct<L>),
    Enum(DataEnum<L>),
}

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

#[derive(Debug, Clone)]
pub struct DataEnum<L: Layout> {
    pub variants: Vec<Statement<L>>,
}

impl<L: Layout> DataEnum<L> {
    /// Iterates over variants, classifying each as wrapper or regular.
    ///
    /// This is useful for derive macros that need to handle wrapper variants
    /// differently from regular variants (e.g., generating match arms).
    ///
    /// # Example
    /// ```ignore
    /// for variant in data.iter_variants() {
    ///     match variant {
    ///         VariantRef::Wrapper { name, wrapper, stmt } => {
    ///             // Generate delegation code
    ///         }
    ///         VariantRef::Regular { name, stmt } => {
    ///             // Generate normal handling code
    ///         }
    ///     }
    /// }
    /// ```
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

/// A reference to an enum variant, classified by whether it's a wrapper or regular variant.
///
/// This is returned by [`DataEnum::iter_variants`] and provides a convenient way
/// to handle the common pattern of treating wrapper and regular variants differently.
#[derive(Debug, Clone, Copy)]
pub enum VariantRef<'a, L: Layout> {
    /// A wrapper variant that delegates to another dialect type.
    Wrapper {
        /// The variant name
        name: &'a syn::Ident,
        /// The wrapper field information
        wrapper: &'a Wrapper,
        /// The full statement (in case you need other fields)
        stmt: &'a Statement<L>,
    },
    /// A regular variant with fields.
    Regular {
        /// The variant name
        name: &'a syn::Ident,
        /// The statement containing all field information
        stmt: &'a Statement<L>,
    },
}

impl<'a, L: Layout> VariantRef<'a, L> {
    /// Returns the variant name.
    pub fn name(&self) -> &'a syn::Ident {
        match self {
            VariantRef::Wrapper { name, .. } => name,
            VariantRef::Regular { name, .. } => name,
        }
    }

    /// Returns the underlying statement.
    pub fn stmt(&self) -> &'a Statement<L> {
        match self {
            VariantRef::Wrapper { stmt, .. } => stmt,
            VariantRef::Regular { stmt, .. } => stmt,
        }
    }

    /// Returns true if this is a wrapper variant.
    pub fn is_wrapper(&self) -> bool {
        matches!(self, VariantRef::Wrapper { .. })
    }
}
