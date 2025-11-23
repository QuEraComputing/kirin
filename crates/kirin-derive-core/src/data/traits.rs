use proc_macro2::TokenStream;

use crate::data::{EnumAttribute, StructAttribute, VariantAttribute};

use super::enum_impl::{EitherEnum, RegularEnum, WrapperEnum};
use super::struct_impl::{NamedWrapperStruct, RegularStruct, UnnamedWrapperStruct};

pub trait StatementFields<'input>: Sized {
    type InfoType: FromStruct<'input, Self> + FromEnum<'input, Self>;
    type FieldsType: FromVariantFields<'input, Self> + FromStructFields<'input, Self>;
}

pub trait HasDefaultCratePath {
    /// Default path to the crate root containing the trait path being derived
    fn default_crate_path(&self) -> syn::Path;
}

pub trait HasTraitGenerics {
    /// Generics for the trait being derived
    fn trait_generics(&self) -> &syn::Generics;

    /// Combine the trait generics with the input type generics
    fn combine_generics(&self, input_generics: &syn::Generics) -> syn::Generics {
        let mut combined = self.trait_generics().clone();
        combined.params.extend(input_generics.params.clone());
        combined
    }
}

pub(super) trait CombinedGenerateFrom<'input>:
    HasTraitGenerics + StatementFields<'input>
    + GenerateFrom<'input, RegularStruct<'input, Self>>
    + GenerateFrom<'input, UnnamedWrapperStruct<'input, Self>>
    + GenerateFrom<'input, NamedWrapperStruct<'input, Self>>
    + GenerateFrom<'input, RegularEnum<'input, Self>>
    + GenerateFrom<'input, WrapperEnum<'input, Self>>
    + GenerateFrom<'input, EitherEnum<'input, Self>>
{
}

impl<'input, T> CombinedGenerateFrom<'input> for T where
    T: HasTraitGenerics + StatementFields<'input>
        + GenerateFrom<'input, RegularStruct<'input, T>>
        + GenerateFrom<'input, UnnamedWrapperStruct<'input, T>>
        + GenerateFrom<'input, NamedWrapperStruct<'input, T>>
        + GenerateFrom<'input, RegularEnum<'input, T>>
        + GenerateFrom<'input, WrapperEnum<'input, T>>
        + GenerateFrom<'input, EitherEnum<'input, T>>
{
}

pub trait GenerateFrom<'input, Data> {
    fn generate_from(&self, data: &Data) -> TokenStream;
}

pub trait FromStruct<'input, T> {
    fn from_struct(
        trait_info: &T,
        attrs: &StructAttribute,
        input: &'input syn::DeriveInput,
    ) -> Self;
}

impl<'input, T> FromStruct<'input, T> for () {
    fn from_struct(
        _trait_info: &T,
        _attrs: &StructAttribute,
        _input: &'input syn::DeriveInput,
    ) -> Self {
        ()
    }
}

pub trait FromEnum<'input, T> {
    fn from_enum(
        trait_info: &T,
        attrs: &EnumAttribute,
        input: &'input syn::DeriveInput,
    ) -> Self;
}

impl<'input, T> FromEnum<'input, T> for () {
    fn from_enum(
        _trait_info: &T,
        _attrs: &EnumAttribute,
        _input: &'input syn::DeriveInput,
    ) -> Self {
        ()
    }
}

/// If the statement is not a wrapper statement,
/// extract relevant info from them
pub trait FromStructFields<'input, T> {
    fn from_struct_fields(
        trait_info: &T,
        attrs: &StructAttribute,
        parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self;
}

impl<'input, T> FromStructFields<'input, T> for () {
    fn from_struct_fields(
        _trait_info: &T,
        _attrs: &StructAttribute,
        _parent: &syn::DataStruct,
        _fields: &syn::Fields,
    ) -> Self {
        ()
    }
}

pub trait FromVariantFields<'input, T> {
    fn from_variant_fields(
        trait_info: &T,
        attrs: &VariantAttribute,
        parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self;
}

impl<'input, T> FromVariantFields<'input, T> for () {
    fn from_variant_fields(
        _trait_info: &T,
        _attrs: &VariantAttribute,
        _parent: &'input syn::Variant,
        _fields: &'input syn::Fields,
    ) -> Self {
        ()
    }
}

pub struct SplitForImpl<'a> {
    pub impl_generics: syn::ImplGenerics<'a>,
    pub trait_ty_generics: syn::TypeGenerics<'a>,
    pub input_ty_generics: syn::TypeGenerics<'a>,
    pub where_clause: Option<syn::WhereClause>,
}

pub trait SplitForImplTrait<'a, T> {
    /// split the generics for use in an trait impl
    fn split_for_impl(&'a self, trait_info: &'a T) -> SplitForImpl<'a>;
}

pub trait CrateRootPath<T> {
    /// Path to the crate root containing the trait path being derived
    fn crate_root_path(&self, trait_info: &T) -> syn::Path;
    fn absolute_path(&self, trait_info: &T, relative_path: &syn::Path) -> syn::Path {
        let mut absolute_path = self.crate_root_path(trait_info);
        absolute_path.segments.extend(relative_path.segments.clone());
        absolute_path
    }
}
