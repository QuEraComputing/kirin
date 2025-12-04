use crate::data::{
    CombineGenerics, CrateRootPath, EnumAttribute, GenerateFrom, HasDefaultCratePath, HasGenerics,
    SplitForImplTrait, StatementFields, VariantAttribute,
};
use proc_macro2::TokenStream;

use super::{either::EitherEnum, regular::RegularEnum, wrapper::WrapperEnum};

pub enum Enum<'input, T: CombineGenerics + StatementFields<'input>> {
    Wrapper(WrapperEnum<'input, T>),
    Either(EitherEnum<'input, T>),
    Regular(RegularEnum<'input, T>),
}

#[bon::bon]
impl<'input, T: CombineGenerics + StatementFields<'input>> Enum<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<EnumAttribute>,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => EnumAttribute::new(input)?,
        };

        let syn::Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "Enum can only be created from enum data",
            ));
        };

        if attrs.wraps {
            return Ok(Self::Wrapper(
                WrapperEnum::builder()
                    .attrs(attrs)
                    .input(input)
                    .trait_info(trait_info)
                    .build()?,
            ));
        } else if data
            .variants
            .iter()
            .map(|variant| VariantAttribute::new(variant))
            .collect::<syn::Result<Vec<_>>>()?
            .iter()
            .any(|variant| variant.is_wrapper())
        {
            return Ok(Self::Either(
                EitherEnum::builder()
                    .attrs(attrs)
                    .input(input)
                    .trait_info(trait_info)
                    .build()?,
            ));
        } else {
            return Ok(Self::Regular(
                RegularEnum::builder()
                    .attrs(attrs)
                    .input(input)
                    .trait_info(trait_info)
                    .build()?,
            ));
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        match self {
            Enum::Wrapper(data) => data.input(),
            Enum::Either(data) => data.input(),
            Enum::Regular(data) => data.input(),
        }
    }

    pub fn type_lattice(&self) -> Option<&syn::Type> {
        match self {
            Enum::Wrapper(data) => data.type_lattice(),
            Enum::Either(data) => data.type_lattice(),
            Enum::Regular(data) => data.type_lattice(),
        }
    }
}

impl<'input, T> std::fmt::Debug for Enum<'input, T>
where
    T: CombineGenerics + StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Enum::Wrapper(data) => f.debug_tuple("EnumTrait::Wrapper").field(data).finish(),
            Enum::Either(data) => f.debug_tuple("EnumTrait::Either").field(data).finish(),
            Enum::Regular(data) => f.debug_tuple("EnumTrait::Regular").field(data).finish(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, Enum<'input, T>> for T
where
    T: CombineGenerics
        + StatementFields<'input>
        + GenerateFrom<'input, WrapperEnum<'input, T>>
        + GenerateFrom<'input, EitherEnum<'input, T>>
        + GenerateFrom<'input, RegularEnum<'input, T>>,
{
    fn generate_from(&self, data: &Enum<'input, T>) -> TokenStream {
        match data {
            Enum::Wrapper(data) => self.generate_from(data),
            Enum::Either(data) => self.generate_from(data),
            Enum::Regular(data) => self.generate_from(data),
        }
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for Enum<'input, T>
where
    T: HasGenerics + StatementFields<'input>,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        match self {
            Enum::Wrapper(data) => data.split_for_impl(trait_info),
            Enum::Either(data) => data.split_for_impl(trait_info),
            Enum::Regular(data) => data.split_for_impl(trait_info),
        }
    }
}

impl<'input, T> CrateRootPath<T> for Enum<'input, T>
where
    T: CombineGenerics + HasDefaultCratePath + StatementFields<'input>,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        match self {
            Enum::Wrapper(data) => data.crate_root_path(trait_info),
            Enum::Either(data) => data.crate_root_path(trait_info),
            Enum::Regular(data) => data.crate_root_path(trait_info),
        }
    }
}
