use std::ops::{Deref, DerefMut};

use darling::FromDeriveInput;

use super::{attrs::GlobalOptions, layout::Layout, statement::Statement};

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
