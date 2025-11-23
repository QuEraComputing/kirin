use proc_macro2::TokenStream;

use super::traits::{TraitInfo, GenerateFrom};

/// some global context for the derive
pub struct Context<'input, T: TraitInfo<'input>> {
    /// information about the trait being derived
    pub trait_info: T,
    /// reference to the input type being derived
    pub input: &'input syn::DeriveInput,
    /// Global attribute data for the trait being derived
    pub data: T::GlobalAttributeData,
    /// combined generics from both the input type and the trait being derived
    pub generics: syn::Generics,
    /// absolute path to the trait being derived
    pub absolute_trait_path: syn::Path,
}

impl<'input, T: TraitInfo<'input>> Context<'input, T> {
    pub fn new(trait_info: T, input: &'input syn::DeriveInput) -> Self {
        let data = T::GlobalAttributeData::default();
        let mut generics = input.generics.clone();
        let trait_generics = trait_info.trait_generics();
        let relative_trait_path = trait_info.relative_trait_path();
        let absolute_trait_path: syn::Path = if let Some(crate_path) = &kirin_attr.crate_path {
            let mut path = crate_path.clone();
            path.segments.extend(relative_trait_path.segments.clone());
            path
        } else {
            let mut path = trait_info.default_crate_path();
            path.segments.extend(relative_trait_path.segments.clone());
            path
        };

        generics.params.extend(trait_generics.params.clone());
        Self {
            trait_info,
            input,
            data,
            kirin_attr,
            generics,
            absolute_trait_path,
        }
    }

    /// splits the generics for impl
    /// - impl_generics: generics for impl declaration
    /// - trait_ty_generics: generics for the type being implemented
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
        let (_, trait_ty_generics, _) = self.trait_info.trait_generics().split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        (
            impl_generics,
            trait_ty_generics,
            input_ty_generics,
            where_clause,
        )
    }
}

impl<'input, T, Data> GenerateFrom<'input, Data> for Context<'input, T>
where
    T: TraitInfo<'input> + GenerateFrom<'input, Data>,
{
    fn generate_from(&self, data: &Data) -> TokenStream {
        self.trait_info.generate_from(data)
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
        self.trait_info.relative_trait_path()
    }

    pub fn method_name(&self) -> &syn::Ident {
        self.trait_info.method_name()
    }
}