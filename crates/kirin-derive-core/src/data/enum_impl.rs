use quote::ToTokens;
use proc_macro2::TokenStream;

use crate::data::TraitInfoGenerateFrom;

use super::{Context, FromVariantFields, GenerateFrom, KirinAttribute, TraitInfo};

pub enum EnumTrait<'input, T: TraitInfo<'input>> {
    Wrapper(WrapperEnum<'input, T>),
    Either(EitherEnum<'input, T>),
    Regular(RegularEnum<'input, T>),
}

impl<'input, T: TraitInfo<'input>> EnumTrait<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        if ctx.kirin_attr.wraps {
            return Self::Wrapper(WrapperEnum::new(ctx, data));
        } else if data.variants.iter().any(|variant| {
            KirinAttribute::from_attrs(&variant.attrs).wraps
                || variant
                    .fields
                    .iter()
                    .any(|field| KirinAttribute::from_field_attrs(&field.attrs).wraps)
        }) {
            return Self::Either(EitherEnum::new(ctx, data));
        } else {
            return Self::Regular(RegularEnum::new(ctx, data));
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for EnumTrait<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnumTrait::Wrapper(data) => f
                .debug_tuple("EnumTrait::Wrapper")
                .field(data)
                .finish(),
            EnumTrait::Either(data) => f
                .debug_tuple("EnumTrait::Either")
                .field(data)
                .finish(),
            EnumTrait::Regular(data) => f
                .debug_tuple("EnumTrait::Regular")
                .field(data)
                .finish(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, EnumTrait<'input, T>> for T
where
    T: TraitInfoGenerateFrom<'input>,
{
    fn generate_from(&self, data: &EnumTrait<'input, T>) -> TokenStream {
        match data {
            EnumTrait::Wrapper(data) => self.generate_from(data),
            EnumTrait::Either(data) => self.generate_from(data),
            EnumTrait::Regular(data) => self.generate_from(data),
        }
    }
}

/// An enum that contains only wrapper instruction definitions.
pub struct WrapperEnum<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub variants: Vec<WrapperVariant<'input, T>>,
}

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
        Self { ctx, variants }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for WrapperEnum<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WrapperEnum")
            .field("variants", &self.variants)
            .finish()
    }
}

/// An enum that contains only regular instruction definitions.
pub struct RegularEnum<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub variants: Vec<RegularVariant<'input, T>>,
}

impl<'input, T: TraitInfo<'input>> RegularEnum<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|variant| RegularVariant::from_fields(ctx, variant, &variant.fields))
            .collect();
        Self { ctx, variants }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for RegularEnum<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegularEnum")
            .field("variants", &self.variants)
            .finish()
    }
}

/// An enum that contains a mix of wrapper and regular instruction definitions.
pub struct EitherEnum<'input, T: TraitInfo<'input>> {
    pub ctx: &'input Context<'input, T>,
    pub variants: Vec<WrapperOrRegularVariant<'input, T>>,
}

impl<'input, T: TraitInfo<'input>> EitherEnum<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>, data: &'input syn::DataEnum) -> Self {
        let variants = data
            .variants
            .iter()
            .map(|variant| WrapperOrRegularVariant::from_variant(ctx, variant))
            .collect();
        Self { ctx, variants }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for EitherEnum<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EitherEnum")
            .field("variants", &self.variants)
            .finish()
    }
}

pub enum WrapperOrRegularVariant<'input, T: TraitInfo<'input>> {
    Wrapper(WrapperVariant<'input, T>),
    Regular(RegularVariant<'input, T>),
}

impl<'input, T: TraitInfo<'input>> WrapperOrRegularVariant<'input, T> {
    /// Creates a new `EitherWrapperOrRegular` from the given variant.
    pub fn from_variant(ctx: &'input Context<'input, T>, variant: &'input syn::Variant) -> Self {
        if KirinAttribute::from_attrs(&variant.attrs).wraps {
            if let Some(wrapper) = WrapperVariant::try_from_variant(ctx, variant) {
                return WrapperOrRegularVariant::Wrapper(wrapper);
            } else {
                panic!("Variant marked as wrapper but could not be parsed as one");
            }
        } else if variant.fields.iter().any(|field| {
            KirinAttribute::from_field_attrs(&field.attrs).wraps
        }) {
            if let Some(wrapper) = WrapperVariant::try_from_variant(ctx, variant) {
                return WrapperOrRegularVariant::Wrapper(wrapper);
            } else {
                panic!("Variant has a field marked as wrapper but could not be parsed as one");
            }
        } else {
            return WrapperOrRegularVariant::Regular(RegularVariant::from_fields(
                ctx,
                variant,
                &variant.fields,
            ));
        }
    }
}

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for WrapperOrRegularVariant<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapperOrRegularVariant::Wrapper(data) => f
                .debug_tuple("WrapperOrRegularVariant::Wrapper")
                .field(data)
                .finish(),
            WrapperOrRegularVariant::Regular(data) => f
                .debug_tuple("WrapperOrRegularVariant::Regular")
                .field(data)
                .finish(),
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

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for WrapperVariant<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapperVariant::Named(data) => f
                .debug_tuple("WrapperVariant::Named")
                .field(data)
                .finish(),
            WrapperVariant::Unnamed(data) => f
                .debug_tuple("WrapperVariant::Unnamed")
                .field(data)
                .finish(),
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
            .find(|f| KirinAttribute::from_field_attrs(&f.attrs).wraps)
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

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for NamedWrapperVariant<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedWrapperVariant")
            .field("variant_name", &self.variant_name)
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
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
            .find(|(_, f)| KirinAttribute::from_field_attrs(&f.attrs).wraps)
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

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for UnnamedWrapperVariant<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnnamedWrapperVariant")
            .field("variant_name", &self.variant_name)
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
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

impl<'input, T: TraitInfo<'input>> std::fmt::Debug for RegularVariant<'input, T>
where
    T::MatchingFields: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegularVariant")
            .field("variant_name", &self.variant_name)
            .field("matching_fields", &self.matching_fields)
            .finish()
    }
}
