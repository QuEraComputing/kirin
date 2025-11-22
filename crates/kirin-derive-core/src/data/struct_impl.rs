use crate::data::TraitInfoGenerateFrom;

use super::{Context, FromStructFields, GenerateFrom, KirinAttribute, TraitInfo};

use proc_macro2::TokenStream;
use quote::ToTokens;

pub enum StructTrait<'input, T: TraitInfo<'input>> {
    Wrapper(WrapperStruct<'input, T>),
    Regular(RegularStruct<'input, T>),
}

impl<'input, T: TraitInfo<'input>> StructTrait<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataStruct) -> Self {
        if let Some(wrapper) = WrapperStruct::try_from_data(ctx, data) {
            Self::Wrapper(wrapper)
        } else {
            Self::Regular(RegularStruct::from_fields(ctx, data, &data.fields))
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for StructTrait<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StructTrait::Wrapper(data) => f
                .debug_tuple("StructTrait::Wrapper")
                .field(data)
                .finish(),
            StructTrait::Regular(data) => f
                .debug_tuple("StructTrait::Regular")
                .field(data)
                .finish(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, StructTrait<'input, T>> for T
where
    T: TraitInfoGenerateFrom<'input>,
{
    fn generate_from(&self, data: &StructTrait<'input, T>) -> TokenStream {
        match data {
            StructTrait::Wrapper(data) => self.generate_from(data),
            StructTrait::Regular(data) => self.generate_from(data),
        }
    }
}

pub struct RegularStruct<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub fields: T::MatchingFields,
}

impl<'input, T: TraitInfo<'input>> RegularStruct<'input, T> {
    pub fn from_fields(
        ctx: &'input Context<'input, T>,
        parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self {
        RegularStruct {
            ctx,
            fields: T::MatchingFields::from_struct_fields(ctx, parent, fields),
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for RegularStruct<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegularStruct")
            .field("fields", &self.fields)
            .finish()
    }
}

pub enum WrapperStruct<'input, T: TraitInfo<'input>> {
    Named(NamedWrapperStruct<'input, T>),
    Unnamed(UnnamedWrapperStruct<'input, T>),
}

impl<'input, T: TraitInfo<'input>> WrapperStruct<'input, T> {
    pub fn try_from_data(
        ctx: &'input Context<'input, T>,
        data: &'input syn::DataStruct,
    ) -> Option<Self> {
        match &data.fields {
            syn::Fields::Named(fields) => Some(Self::Named(NamedWrapperStruct::try_from_fields(
                ctx, fields,
            )?)),
            syn::Fields::Unnamed(fields) => Some(Self::Unnamed(
                UnnamedWrapperStruct::try_from_fields(ctx, fields)?,
            )),
            _ => None,
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for WrapperStruct<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapperStruct::Named(data) => f
                .debug_tuple("WrapperStruct::Named")
                .field(data)
                .finish(),
            WrapperStruct::Unnamed(data) => f
                .debug_tuple("WrapperStruct::Unnamed")
                .field(data)
                .finish(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, WrapperStruct<'input, T>> for T
where
    T: TraitInfoGenerateFrom<'input>,
{
    fn generate_from(&self, data: &WrapperStruct<'input, T>) -> TokenStream {
        match data {
            WrapperStruct::Named(data) => self.generate_from(data),
            WrapperStruct::Unnamed(data) => self.generate_from(data),
        }
    }
}

pub struct NamedWrapperStruct<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
}

impl<'input, T: TraitInfo<'input>> NamedWrapperStruct<'input, T> {
    pub fn try_from_fields(
        ctx: &'input Context<'input, T>,
        fields: &'input syn::FieldsNamed,
    ) -> Option<Self> {
        if fields.named.len() == 1 {
            let f = fields.named.first().unwrap();
            Some(NamedWrapperStruct {
                ctx,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            })
        } else if let Some(f) = fields
            .named
            .iter()
            .find(|f| KirinAttribute::from_field_attrs(&f.attrs).wraps)
        {
            Some(NamedWrapperStruct {
                ctx,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            })
        } else {
            None
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for NamedWrapperStruct<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedWrapperStruct")
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
    }
}

pub struct UnnamedWrapperStruct<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub wraps: usize,
    pub wraps_type: syn::Type,
}

impl<'input, T: TraitInfo<'input>> UnnamedWrapperStruct<'input, T> {
    pub fn try_from_fields(
        ctx: &'input Context<'input, T>,
        fields: &'input syn::FieldsUnnamed,
    ) -> Option<Self> {
        if fields.unnamed.len() == 1 {
            let f = fields.unnamed.first().unwrap();
            Some(UnnamedWrapperStruct {
                ctx,
                wraps: 0,
                wraps_type: f.ty.clone(),
            })
        } else if let Some((index, f)) = fields
            .unnamed
            .iter()
            .enumerate()
            .find(|(_, f)| KirinAttribute::from_field_attrs(&f.attrs).wraps)
        {
            Some(UnnamedWrapperStruct {
                ctx,
                wraps: index,
                wraps_type: f.ty.clone(),
            })
        } else {
            None
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for UnnamedWrapperStruct<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnnamedWrapperStruct")
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
    }
}
