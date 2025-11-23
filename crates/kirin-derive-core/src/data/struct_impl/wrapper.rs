use crate::data::{
    CrateRootPath, GenerateFrom, HasDefaultCratePath, HasTraitGenerics, SplitForImplTrait,
    StructAttribute,
};

use proc_macro2::TokenStream;

pub enum WrapperStruct<'input, T: HasTraitGenerics> {
    Named(NamedWrapperStruct<'input, T>),
    Unnamed(UnnamedWrapperStruct<'input, T>),
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> WrapperStruct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self {
        let syn::Data::Struct(data) = &input.data else {
            panic!("WrapperStruct can only be created from struct data");
        };

        match &data.fields {
            syn::Fields::Named(_) => Self::Named(
                NamedWrapperStruct::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .input(input)
                    .build(),
            ),
            syn::Fields::Unnamed(_) => Self::Unnamed(
                UnnamedWrapperStruct::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .input(input)
                    .build(),
            ),
            _ => panic!(
                "WrapperStruct can only be created from named or unnamed fields, got unit struct"
            ),
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        match self {
            WrapperStruct::Named(data) => data.input(),
            WrapperStruct::Unnamed(data) => data.input(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, WrapperStruct<'input, T>> for T
where
    T: HasTraitGenerics
        + GenerateFrom<'input, UnnamedWrapperStruct<'input, T>>
        + GenerateFrom<'input, NamedWrapperStruct<'input, T>>,
{
    fn generate_from(&self, data: &WrapperStruct<'input, T>) -> TokenStream {
        match data {
            WrapperStruct::Named(data) => self.generate_from(data),
            WrapperStruct::Unnamed(data) => self.generate_from(data),
        }
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for WrapperStruct<'input, T>
where
    T: HasTraitGenerics,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        match self {
            WrapperStruct::Named(data) => data.split_for_impl(trait_info),
            WrapperStruct::Unnamed(data) => data.split_for_impl(trait_info),
        }
    }
}

impl<'input, T> CrateRootPath<T> for WrapperStruct<'input, T>
where
    T: HasDefaultCratePath + HasTraitGenerics,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        match self {
            WrapperStruct::Named(data) => data.crate_root_path(trait_info),
            WrapperStruct::Unnamed(data) => data.crate_root_path(trait_info),
        }
    }
}

pub struct NamedWrapperStruct<'input, T: HasTraitGenerics> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: StructAttribute,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> NamedWrapperStruct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self
    where
        T: HasTraitGenerics,
    {
        let attrs = attrs.unwrap_or_else(|| StructAttribute::new(input));
        let combined_generics = trait_info.combine_generics(&input.generics);

        let syn::Data::Struct(data) = &input.data else {
            panic!("NamedWrapperStruct can only be created from struct data");
        };

        let syn::Fields::Named(fields) = &data.fields else {
            panic!("NamedWrapperStruct can only be created from named fields");
        };

        if attrs.wraps && fields.named.len() == 1 {
            let f = fields.named.first().unwrap();
            return NamedWrapperStruct {
                input,
                combined_generics,
                attrs,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            };
        }

        if let Some(field_attrs) = &attrs.fields {
            for (f, f_attr) in fields.named.iter().zip(field_attrs.iter()) {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return NamedWrapperStruct {
                            input,
                            combined_generics,
                            attrs,
                            wraps: f.ident.clone().unwrap(),
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        };
                    }
                }
            }
        }
        panic!(
            "Struct is marked as wrapper but no field marked as wrapper or no single field present"
        );
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        self.input
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for NamedWrapperStruct<'input, T>
where
    T: HasTraitGenerics,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.trait_generics().split_for_impl();
        crate::data::SplitForImpl {
            impl_generics,
            trait_ty_generics,
            input_ty_generics,
            where_clause: where_clause.cloned(),
        }
    }
}

impl<'input, T> CrateRootPath<T> for NamedWrapperStruct<'input, T>
where
    T: HasDefaultCratePath + HasTraitGenerics,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}

pub struct UnnamedWrapperStruct<'input, T: HasTraitGenerics> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: StructAttribute,
    pub wraps: usize,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> UnnamedWrapperStruct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| StructAttribute::new(input));
        let combined_generics = trait_info.combine_generics(&input.generics);

        let syn::Data::Struct(data) = &input.data else {
            panic!("UnnamedWrapperStruct can only be created from struct data");
        };

        let syn::Fields::Unnamed(fields) = &data.fields else {
            panic!("UnnamedWrapperStruct can only be created from unnamed fields");
        };

        if attrs.wraps && fields.unnamed.len() == 1 {
            let f = fields.unnamed.iter().next().unwrap();
            return Self {
                input,
                combined_generics,
                attrs,
                wraps: 0,
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            };
        }

        if let Some(field_attrs) = &attrs.fields {
            for (i, (f, f_attr)) in fields.unnamed.iter().zip(field_attrs.iter()).enumerate() {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return Self {
                            input,
                            combined_generics,
                            attrs,
                            wraps: i,
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        };
                    }
                }
            }
        }
        panic!(
            "NamedWrapperStruct::from_fields called on non-wrapper struct, no field marked as wrapper or no single field present"
        );
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        self.input
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for UnnamedWrapperStruct<'input, T>
where
    T: HasTraitGenerics,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.trait_generics().split_for_impl();
        crate::data::SplitForImpl {
            impl_generics,
            trait_ty_generics,
            input_ty_generics,
            where_clause: where_clause.cloned(),
        }
    }
}

impl<'input, T> CrateRootPath<T> for UnnamedWrapperStruct<'input, T>
where
    T: HasDefaultCratePath + HasTraitGenerics,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}
