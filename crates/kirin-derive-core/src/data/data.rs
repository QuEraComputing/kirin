use proc_macro2::TokenStream;

use crate::data::{CrateRootPath, HasDefaultCratePath, SplitForImplTrait};

use super::enum_impl::Enum;
use super::struct_impl::Struct;
use super::traits::{GenerateFrom, HasTraitGenerics, StatementFields};

pub enum Data<'input, T: HasTraitGenerics + StatementFields<'input>> {
    Struct(Struct<'input, T>),
    Enum(Enum<'input, T>),
}

#[bon::bon]
impl<'input, T: HasTraitGenerics + StatementFields<'input>> Data<'input, T> {
    #[builder]
    pub fn new(trait_info: &T, input: &'input syn::DeriveInput) -> Self {
        match &input.data {
            syn::Data::Struct(_) => Data::Struct(
                Struct::builder()
                    .trait_info(trait_info)
                    .input(input)
                    .build(),
            ),
            syn::Data::Enum(_) => {
                Data::Enum(Enum::builder().trait_info(trait_info).input(input).build())
            }
            _ => panic!("only structs and enums are supported"),
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        match self {
            Data::Struct(data) => data.input(),
            Data::Enum(data) => data.input(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, Data<'input, T>> for T
where
    T: HasTraitGenerics
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
    T: HasTraitGenerics + StatementFields<'input>,
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
    T: HasTraitGenerics + StatementFields<'input> + HasDefaultCratePath,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        match self {
            Data::Struct(data) => data.crate_root_path(trait_info),
            Data::Enum(data) => data.crate_root_path(trait_info),
        }
    }
}
