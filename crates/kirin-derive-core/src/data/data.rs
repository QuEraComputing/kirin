use proc_macro2::TokenStream;

use crate::data::{CombineGenerics, CrateRootPath, HasDefaultCratePath, SplitForImplTrait};

use super::enum_impl::Enum;
use super::struct_impl::Struct;
use super::traits::{GenerateFrom, HasGenerics, StatementFields};

pub enum Data<'input, T: CombineGenerics + StatementFields<'input>> {
    Struct(Struct<'input, T>),
    Enum(Enum<'input, T>),
}

#[bon::bon]
impl<'input, T: CombineGenerics + StatementFields<'input>> Data<'input, T> {
    #[builder]
    pub fn new(trait_info: &T, input: &'input syn::DeriveInput) -> syn::Result<Self> {
        match &input.data {
            syn::Data::Struct(_) => Ok(Data::Struct(
                Struct::builder()
                    .trait_info(trait_info)
                    .input(input)
                    .build()?,
            )),
            syn::Data::Enum(_) => Ok(Data::Enum(
                Enum::builder()
                    .trait_info(trait_info)
                    .input(input)
                    .build()?,
            )),
            _ => Err(syn::Error::new_spanned(
                input,
                "Data can only be created from struct or enum data",
            )),
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        match self {
            Data::Struct(data) => data.input(),
            Data::Enum(data) => data.input(),
        }
    }

    pub fn type_lattice(&self) -> Option<&syn::Type> {
        match self {
            Data::Struct(data) => data.type_lattice(),
            Data::Enum(data) => data.type_lattice(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, Data<'input, T>> for T
where
    T: CombineGenerics
        + StatementFields<'input>
        + GenerateFrom<'input, Struct<'input, T>>
        + GenerateFrom<'input, Enum<'input, T>>,
{
    fn generate_from(&self, data: &Data<'input, T>) -> TokenStream {
        match data {
            Data::Struct(data) => self.generate_from(data),
            Data::Enum(data) => self.generate_from(data),
        }
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for Data<'input, T>
where
    T: HasGenerics + StatementFields<'input>,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> super::SplitForImpl<'a> {
        match self {
            Data::Struct(data) => data.split_for_impl(trait_info),
            Data::Enum(data) => data.split_for_impl(trait_info),
        }
    }
}

impl<'input, T> CrateRootPath<T> for Data<'input, T>
where
    T: HasGenerics + StatementFields<'input> + HasDefaultCratePath,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        match self {
            Data::Struct(data) => data.crate_root_path(trait_info),
            Data::Enum(data) => data.crate_root_path(trait_info),
        }
    }
}

impl<'input, T> std::fmt::Debug for Data<'input, T>
where
    T: CombineGenerics + StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
    T::InfoType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Data::Struct(data) => f.debug_tuple("Data::Struct").field(data).finish(),
            Data::Enum(data) => f.debug_tuple("Data::Enum").field(data).finish(),
        }
    }
}
