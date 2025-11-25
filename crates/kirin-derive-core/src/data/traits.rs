use proc_macro2::TokenStream;

use crate::data::{EnumAttribute, StructAttribute, VariantAttribute};

pub trait StatementFields<'input>: Sized {
    type InfoType: FromStruct<'input, Self> + FromEnum<'input, Self>;
    type FieldsType: FromVariantFields<'input, Self> + FromStructFields<'input, Self>;
}

pub trait HasDefaultCratePath {
    /// Default path to the crate root containing the trait path being derived
    fn default_crate_path(&self) -> syn::Path;
}

pub trait HasGenerics {
    /// Generics for the trait being derived
    fn generics(&self) -> &syn::Generics;
}

pub trait CombineGenerics {
    /// combine the generics of self with other
    fn combine_generics(&self, other: &syn::Generics) -> syn::Generics;
}

impl<T: HasGenerics> CombineGenerics for T {
    fn combine_generics(&self, other: &syn::Generics) -> syn::Generics {
        let mut combined = self.generics().clone();
        combined.params.extend(other.params.clone());
        combined
    }
}

pub trait GenerateFrom<'input, Data> {
    fn generate_from(&self, data: &Data) -> TokenStream;
}

impl<'a, T, Data> GenerateFrom<'a, syn::Result<Data>> for T
where
    T: GenerateFrom<'a, Data>,
{
    fn generate_from(&self, data: &syn::Result<Data>) -> TokenStream {
        match data {
            Ok(d) => self.generate_from(d),
            Err(e) => e.to_compile_error(),
        }
    }
}

pub trait FromStruct<'input, T>: Sized {
    fn from_struct(
        trait_info: &T,
        attrs: &StructAttribute,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self>;
}

impl<'input, T> FromStruct<'input, T> for () {
    fn from_struct(
        _trait_info: &T,
        _attrs: &StructAttribute,
        _input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        Ok(())
    }
}

pub trait FromEnum<'input, T>: Sized {
    fn from_enum(
        trait_info: &T,
        attrs: &EnumAttribute,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self>;
}

impl<'input, T> FromEnum<'input, T> for () {
    fn from_enum(
        _trait_info: &T,
        _attrs: &EnumAttribute,
        _input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        Ok(())
    }
}

/// If the statement is not a wrapper statement,
/// extract relevant info from them
pub trait FromStructFields<'input, T>: Sized {
    fn from_struct_fields(
        trait_info: &T,
        attrs: &StructAttribute,
        parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> syn::Result<Self>;
}

impl<'input, T> FromStructFields<'input, T> for () {
    fn from_struct_fields(
        _trait_info: &T,
        _attrs: &StructAttribute,
        _parent: &syn::DataStruct,
        _fields: &syn::Fields,
    ) -> syn::Result<Self> {
        Ok(())
    }
}

pub trait FromVariantFields<'input, T>: Sized {
    fn from_variant_fields(
        trait_info: &T,
        attrs: &VariantAttribute,
        parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> syn::Result<Self>;
}

impl<'input, T> FromVariantFields<'input, T> for () {
    fn from_variant_fields(
        _trait_info: &T,
        _attrs: &VariantAttribute,
        _parent: &'input syn::Variant,
        _fields: &'input syn::Fields,
    ) -> syn::Result<Self> {
        Ok(())
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
        absolute_path
            .segments
            .extend(relative_path.segments.clone());
        absolute_path
    }
}
