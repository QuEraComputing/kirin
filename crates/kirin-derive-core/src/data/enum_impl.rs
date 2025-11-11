use proc_macro2::TokenStream;

use super::{Context, FromVariantFields, GenerateFrom, TraitInfo};
use crate::has_attr;

pub enum EnumTrait<'input, T: TraitInfo<'input>> {
    Wrapper(WrapperEnum<'input, T>),
    Either(EitherEnum<'input, T>),
    Regular(RegularEnum<'input, T>),
}

impl<'input, T: TraitInfo<'input>> EnumTrait<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        if ctx.wraps {
            return Self::Wrapper(WrapperEnum::new(ctx, data));
        } else if data.variants.iter().any(|variant| {
            has_attr(&variant.attrs, "kirin", "wraps")
                || variant
                    .fields
                    .iter()
                    .any(|field| has_attr(&field.attrs, "kirin", "wraps"))
        }) {
            return Self::Either(EitherEnum::new(ctx, data));
        } else {
            return Self::Regular(RegularEnum::new(ctx, data));
        }
    }
}

impl<'input, T> GenerateFrom<'input, EnumTrait<'input, T>> for T
where
    T: TraitInfo<'input>
        + GenerateFrom<'input, WrapperEnum<'input, T>>
        + GenerateFrom<'input, EitherEnum<'input, T>>
        + GenerateFrom<'input, RegularEnum<'input, T>>,
{
    fn generate_from(&self, data: &EnumTrait<'input, T>) -> TokenStream {
        match data {
            EnumTrait::Wrapper(data) => self.generate_from(data),
            EnumTrait::Either(data) => self.generate_from(data),
            EnumTrait::Regular(data) => self.generate_from(data),
        }
    }
}

pub struct WrapperEnum<'input, T: TraitInfo<'input>>(pub Vec<WrapperVariant<'input, T>>);

impl<'input, T: TraitInfo<'input>> WrapperEnum<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        let variants = data
                .variants
                .iter()
                .map(|variant| {
                    WrapperVariant::try_from_variant(ctx, variant)
                        .expect("all variants must be wrapper variants when #[kirin(wraps)] is used on the enum")
                })
                .collect();
        Self(variants)
    }
}

pub struct RegularEnum<'input, T: TraitInfo<'input>>(pub Vec<RegularVariant<'input, T>>);

impl<'input, T: TraitInfo<'input>> RegularEnum<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|variant| RegularVariant::from_fields(ctx, variant, &variant.fields))
            .collect();
        Self(variants)
    }
}

pub struct EitherEnum<'input, T: TraitInfo<'input>>(pub Vec<EitherWrapperOrRegular<'input, T>>);

impl<'input, T: TraitInfo<'input>> EitherEnum<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|variant| EitherWrapperOrRegular::from_variant(ctx, variant))
            .collect();
        Self(variants)
    }
}

pub enum EitherWrapperOrRegular<'input, T: TraitInfo<'input>> {
    Wrapper(WrapperVariant<'input, T>),
    Regular(RegularVariant<'input, T>),
}

impl<'input, T: TraitInfo<'input>> EitherWrapperOrRegular<'input, T> {
    /// Creates a new `EitherWrapperOrRegular` from the given variant.
    pub fn from_variant(ctx: &'input Context<'input, T>, variant: &'input syn::Variant) -> Self {
        if let Some(wrapper) = WrapperVariant::try_from_variant(ctx, variant) {
            EitherWrapperOrRegular::Wrapper(wrapper)
        } else {
            EitherWrapperOrRegular::Regular(RegularVariant::from_fields(
                ctx,
                variant,
                &variant.fields,
            ))
        }
    }
}

impl<'input, T> GenerateFrom<'input, EitherWrapperOrRegular<'input, T>> for T
where
    T: TraitInfo<'input>
        + GenerateFrom<'input, WrapperVariant<'input, T>>
        + GenerateFrom<'input, RegularVariant<'input, T>>,
{
    fn generate_from(&self, data: &EitherWrapperOrRegular<'input, T>) -> TokenStream {
        match data {
            EitherWrapperOrRegular::Wrapper(data) => self.generate_from(data),
            EitherWrapperOrRegular::Regular(data) => self.generate_from(data),
        }
    }
}

pub enum WrapperVariant<'input, T: TraitInfo<'input>> {
    Named(NamedWrapperVariant<'input, T>),
    Unnamed(UnnamedWrapperVariant<'input, T>),
}

impl<'input, T: TraitInfo<'input>> WrapperVariant<'input, T> {
    /// Creates a new `WrapperVariant` from the given variant if it is a wrapper variant.
    pub fn try_from_variant(
        ctx: &'input Context<'input, T>,
        variant: &'input syn::Variant,
    ) -> Option<Self> {
        match &variant.fields {
            syn::Fields::Named(fields) => Some(WrapperVariant::Named(
                NamedWrapperVariant::try_from_fields(ctx, &variant, fields)?,
            )),
            syn::Fields::Unnamed(fields) => Some(WrapperVariant::Unnamed(
                UnnamedWrapperVariant::try_from_fields(ctx, &variant, fields)?,
            )),
            _ => None,
        }
    }
}

impl<'input, T> GenerateFrom<'input, WrapperVariant<'input, T>> for T
where
    T: TraitInfo<'input>
        + GenerateFrom<'input, NamedWrapperVariant<'input, T>>
        + GenerateFrom<'input, UnnamedWrapperVariant<'input, T>>,
{
    fn generate_from(&self, data: &WrapperVariant<'input, T>) -> TokenStream {
        match data {
            WrapperVariant::Named(data) => self.generate_from(data),
            WrapperVariant::Unnamed(data) => self.generate_from(data),
        }
    }
}

pub struct NamedWrapperVariant<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub variant_name: &'input syn::Ident,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
}

impl<'input, T: TraitInfo<'input>> NamedWrapperVariant<'input, T> {
    pub fn try_from_fields(
        ctx: &'input Context<'input, T>,
        parent: &'input syn::Variant,
        fields: &'input syn::FieldsNamed,
    ) -> Option<Self> {
        if fields.named.len() == 1 {
            let f = fields.named.first().unwrap();
            Some(Self {
                ctx,
                variant_name: &parent.ident,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            })
        } else if let Some(f) = fields
            .named
            .iter()
            .find(|f| has_attr(&f.attrs, "kirin", "wraps"))
        {
            Some(Self {
                ctx,
                variant_name: &parent.ident,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            })
        } else {
            None
        }
    }
}

pub struct UnnamedWrapperVariant<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub variant_name: &'input syn::Ident,
    pub wraps: usize,
    pub wraps_type: syn::Type,
}

impl<'input, T: TraitInfo<'input>> UnnamedWrapperVariant<'input, T> {
    pub fn try_from_fields(
        ctx: &'input Context<'input, T>,
        parent: &'input syn::Variant,
        fields: &'input syn::FieldsUnnamed,
    ) -> Option<Self> {
        if fields.unnamed.len() == 1 {
            Some(Self {
                ctx,
                variant_name: &parent.ident,
                wraps: 0,
                wraps_type: fields.unnamed.first().unwrap().ty.clone(),
            })
        } else if let Some((index, f)) = fields
            .unnamed
            .iter()
            .enumerate()
            .find(|(_, f)| has_attr(&f.attrs, "kirin", "wraps"))
        {
            Some(Self {
                ctx,
                variant_name: &parent.ident,
                wraps: index,
                wraps_type: f.ty.clone(),
            })
        } else {
            None
        }
    }
}

pub struct RegularVariant<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub variant_name: &'input syn::Ident,
    pub matching_fields: T::MatchingFields,
}

impl<'input, T: TraitInfo<'input>> RegularVariant<'input, T> {
    pub fn from_fields(
        ctx: &'input Context<'input, T>,
        parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self {
        Self {
            ctx,
            variant_name: &parent.ident,
            matching_fields: T::MatchingFields::from_variant_fields(ctx, parent, fields),
        }
    }
}
