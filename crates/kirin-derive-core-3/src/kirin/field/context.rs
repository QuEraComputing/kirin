use crate::kirin::attrs::{
    KirinEnumOptions, KirinFieldOptions, KirinStructOptions, KirinVariantOptions,
};
use crate::prelude::*;
use bon::Builder;

use super::{enum_impl::EnumImpl, extra::FieldExtra, struct_impl::StructImpl};

/// context information for deriving field iterators with following signature:
///
/// ```ignore
/// pub trait <TraitName><'trait_lifetime, ...> {
///    type <IterName>: Iterator<Item = &mut <MatchingTypePath>>;
///    fn <TraitMethod>(&'trait_lifetime self) -> Self::<IterName>;
/// }
/// ```
#[derive(Clone, Builder)]
pub struct FieldsIter {
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub default_crate_path: syn::Path,
    #[builder(default)]
    pub mutable: bool,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_path: syn::Path,
    #[builder(default = strip_path(&trait_path))]
    pub trait_name: syn::Ident,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_lifetime: syn::Lifetime,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_type_iter: syn::Ident,
    // #[builder(default, with = |s: impl Into<String>| from_str(s))]
    // pub trait_generics: syn::Generics,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_method: syn::Ident,
    /// name of the iterator to generate
    #[builder(
        default = quote::format_ident!("{}Iter", to_camel_case(&trait_method.to_string())),
        with = |s: impl Into<String>| from_str(s)
    )]
    pub iter_name: syn::Ident,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub matching_type: syn::Path,
    #[builder(default = strip_path(&matching_type))]
    pub matching_type_name: syn::Ident,
    #[builder(default = {
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                trait_lifetime.clone(),
            )));
        generics
    })]
    pub generics: syn::Generics,
}

impl Layout for FieldsIter {
    type EnumAttr = KirinEnumOptions;
    type StructAttr = KirinStructOptions;
    type VariantAttr = KirinVariantOptions;
    type FieldAttr = KirinFieldOptions;
    type StatementExtra = ();
    type FieldExtra = FieldExtra;
}

impl DeriveWithCratePath for FieldsIter {
    fn default_crate_path(&self) -> &syn::Path {
        &self.default_crate_path
    }
}

impl DeriveTrait for FieldsIter {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl DeriveTraitWithGenerics for FieldsIter {
    fn generics(&self) -> &syn::Generics {
        &self.generics
    }
}

impl<'src> Emit<'src> for FieldsIter {
    type EnumImpl = EnumImpl;
    type StructImpl = StructImpl;
}
