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
    pub raw_attrs: Vec<syn::Attribute>,
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
                raw_attrs: input.attrs.clone(),
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
                raw_attrs: input.attrs.clone(),
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
    pub fn name(&self) -> &'a syn::Ident {
        match self {
            VariantRef::Wrapper { name, .. } => name,
            VariantRef::Regular { name, .. } => name,
        }
    }

    pub fn stmt(&self) -> &'a Statement<L> {
        match self {
            VariantRef::Wrapper { stmt, .. } => stmt,
            VariantRef::Regular { stmt, .. } => stmt,
        }
    }

    pub fn is_wrapper(&self) -> bool {
        matches!(self, VariantRef::Wrapper { .. })
    }
}
