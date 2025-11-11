use proc_macro2::TokenStream;

use crate::has_attr;

pub trait TraitInfo<'input>: Sized {
    type GlobalAttributeData: Default;
    type MatchingFields: FromVariantFields<'input, Self>
        + FromStructFields<'input, Self>;
    fn trait_path(&self) -> &syn::Path;
    fn trait_generics(&self) -> &syn::Generics;
    fn method_name(&self) -> &syn::Ident;
}

// pub struct CheckerTraitInfo {
//     pub trait_path: syn::Path,
// }

// impl<'input> TraitInfo<'input> for CheckerTraitInfo {
//     type GlobalAttributeData = bool;
//     type MatchingFields = bool;
//     fn trait_path(&self) -> syn::Path {
//         self.trait_path.clone()
//     }
// }

// impl FromFields<'_, CheckerTraitInfo> for bool {
//     fn from_fields(
//             ctx: &Context<'_, CheckerTraitInfo>,
//             parent: &'_ syn::Variant,
//             fields: &'_ syn::Fields,
//         ) -> Self {
//         true
//     }
// }
pub trait GenerateFrom<'input, Data>: TraitInfo<'input> {
    fn generate_from(&self, data: &Data) -> TokenStream;
}

pub trait FromStructFields<'input, T: TraitInfo<'input>> {
    fn from_struct_fields(
        ctx: &Context<'input, T>,
        parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self;
}

pub trait FromVariantFields<'input, T: TraitInfo<'input>> {
    fn from_variant_fields(
        ctx: &Context<'input, T>,
        parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self;
}

/// some global context for the derive
pub struct Context<'input, T: TraitInfo<'input>> {
    pub trait_info: T,
    pub input: &'input syn::DeriveInput,
    pub data: T::GlobalAttributeData,
    /// if there is a global #[kirin(wraps)] attribute on the enum
    pub wraps: bool,
    pub generics: syn::Generics,
}

impl<'input, T: TraitInfo<'input>> Context<'input, T> {
    pub fn new(trait_info: T, input: &'input syn::DeriveInput) -> Self {
        let wraps = has_attr(&input.attrs, "kirin", "wraps");
        let data = T::GlobalAttributeData::default();
        let mut generics = input.generics.clone();
        let trait_generics = trait_info.trait_generics();

        generics.params.extend(trait_generics.params.clone());
        Self {
            trait_info,
            input,
            data,
            wraps,
            generics,
        }
    }

    /// splits the generics for impl
    /// - impl_generics: generics for impl declaration
    /// - ty_generics: generics for the type being implemented
    /// - input_type_generics: generics for the input type
    /// - where_clause: where clause
    pub fn split_for_impl(
        &'input self,
    ) -> (
        syn::ImplGenerics<'input>,
        syn::TypeGenerics<'input>,
        syn::TypeGenerics<'input>,
        Option<&'input syn::WhereClause>,
    ) {
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        (impl_generics, ty_generics, input_ty_generics, where_clause)
    }
}

impl<'input, T: TraitInfo<'input> + Default> Context<'input, T> {
    pub fn from_input(input: &'input syn::DeriveInput) -> Self {
        Self::new(T::default(), input)
    }

    /// name of the type being derived
    pub fn name(&self) -> &syn::Ident {
        &self.input.ident
    }

    pub fn trait_path(&self) -> &syn::Path {
        self.trait_info.trait_path()
    }

    pub fn method_name(&self) -> &syn::Ident {
        self.trait_info.method_name()
    }
}

mod enum_impl;
mod struct_impl;

pub enum DataTrait<'input, T: TraitInfo<'input>> {
    Struct(struct_impl::StructTrait<'input, T>),
    Enum(enum_impl::EnumTrait<'input, T>),
}

impl<'input, T: TraitInfo<'input>> DataTrait<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>) -> Self {
        match &ctx.input.data {
            syn::Data::Struct(data) => DataTrait::Struct(struct_impl::StructTrait::new(ctx, data)),
            syn::Data::Enum(data) => DataTrait::Enum(enum_impl::EnumTrait::new(ctx, data)),
            _ => panic!("only structs and enums are supported"),
        }
    }
}

impl<'input, T> GenerateFrom<'input, DataTrait<'input, T>> for T
where 
    T: TraitInfo<'input>
        + GenerateFrom<'input, struct_impl::StructTrait<'input, T>>
        + GenerateFrom<'input, enum_impl::EnumTrait<'input, T>>,
{
    fn generate_from(&self, data: &DataTrait<'input, T>) -> TokenStream {
        match data {
            DataTrait::Struct(data) => self.generate_from(data),
            DataTrait::Enum(data) => self.generate_from(data),
        }
    }
}
