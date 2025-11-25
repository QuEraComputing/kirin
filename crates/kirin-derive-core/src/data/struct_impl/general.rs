use super::regular::RegularStruct;
use super::wrapper::WrapperStruct;
use crate::data::{
    CombineGenerics, CrateRootPath, GenerateFrom, HasDefaultCratePath, HasGenerics, SplitForImplTrait, StatementFields, StructAttribute
};

use proc_macro2::TokenStream;

pub enum Struct<'input, T: CombineGenerics + StatementFields<'input>> {
    Wrapper(WrapperStruct<'input, T>),
    Regular(RegularStruct<'input, T>),
}

#[bon::bon]
impl<'input, T: CombineGenerics + StatementFields<'input>> Struct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| StructAttribute::new(input));
        if attrs.is_wrapper() {
            Self::Wrapper(
                WrapperStruct::builder()
                    .trait_info(trait_info)
                    .attrs(attrs)
                    .input(input)
                    .build(),
            )
        } else {
            Self::Regular(
                RegularStruct::builder()
                    .trait_info(trait_info)
                    .attrs(attrs)
                    .input(input)
                    .build(),
            )
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        match self {
            Struct::Wrapper(data) => data.input(),
            Struct::Regular(data) => data.input(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, Struct<'input, T>> for T
where
    T: HasGenerics
        + StatementFields<'input>
        + GenerateFrom<'input, WrapperStruct<'input, T>>
        + GenerateFrom<'input, RegularStruct<'input, T>>,
{
    fn generate_from(&self, data: &Struct<'input, T>) -> TokenStream {
        match data {
            Struct::Wrapper(data) => self.generate_from(data),
            Struct::Regular(data) => self.generate_from(data),
        }
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for Struct<'input, T>
where
    T: HasGenerics + StatementFields<'input>,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        match self {
            Struct::Wrapper(data) => data.split_for_impl(trait_info),
            Struct::Regular(data) => data.split_for_impl(trait_info),
        }
    }
}

impl<'input, T> CrateRootPath<T> for Struct<'input, T>
where
    T: HasDefaultCratePath + HasGenerics + StatementFields<'input>,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        match self {
            Struct::Wrapper(data) => data.crate_root_path(trait_info),
            Struct::Regular(data) => data.crate_root_path(trait_info),
        }
    }
}

impl<'input, T> std::fmt::Debug for Struct<'input, T>
where
    T: CombineGenerics + StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
    T::InfoType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Struct::Wrapper(data) => f.debug_tuple("StructTrait::Wrapper").field(data).finish(),
            Struct::Regular(data) => f.debug_tuple("StructTrait::Regular").field(data).finish(),
        }
    }
}