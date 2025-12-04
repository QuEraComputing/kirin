use crate::data::{
    CombineGenerics, CrateRootPath, GenerateFrom, HasDefaultCratePath, HasGenerics,
    SplitForImplTrait, StructAttribute,
};

use proc_macro2::TokenStream;

pub enum WrapperStruct<'input, T> {
    Named(NamedWrapperStruct<'input, T>),
    Unnamed(UnnamedWrapperStruct<'input, T>),
}

#[bon::bon]
impl<'input, T: CombineGenerics> WrapperStruct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        let syn::Data::Struct(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "WrapperStruct can only be created from struct data",
            ));
        };

        match &data.fields {
            syn::Fields::Named(_) => Ok(Self::Named(
                NamedWrapperStruct::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .input(input)
                    .build()?,
            )),
            syn::Fields::Unnamed(_) => Ok(Self::Unnamed(
                UnnamedWrapperStruct::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .input(input)
                    .build()?,
            )),
            _ => Err(syn::Error::new_spanned(
                input,
                "WrapperStruct can only be created from named or unnamed struct data",
            )),
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        match self {
            WrapperStruct::Named(data) => data.input(),
            WrapperStruct::Unnamed(data) => data.input(),
        }
    }

    pub fn type_lattice(&self) -> Option<&syn::Type> {
        match self {
            WrapperStruct::Named(data) => data.type_lattice(),
            WrapperStruct::Unnamed(data) => data.type_lattice(),
        }
    }
}

impl<'input, T> GenerateFrom<'input, WrapperStruct<'input, T>> for T
where
    T: HasGenerics
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
    T: HasGenerics,
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
    T: HasDefaultCratePath,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        match self {
            WrapperStruct::Named(data) => data.crate_root_path(trait_info),
            WrapperStruct::Unnamed(data) => data.crate_root_path(trait_info),
        }
    }
}

pub struct NamedWrapperStruct<'input, T> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: StructAttribute,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T: CombineGenerics> NamedWrapperStruct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => StructAttribute::new(input)?,
        };
        let combined_generics = trait_info.combine_generics(&input.generics);

        let syn::Data::Struct(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "NamedWrapperStruct can only be created from struct data",
            ));
        };

        let syn::Fields::Named(fields) = &data.fields else {
            return Err(syn::Error::new_spanned(
                input,
                "NamedWrapperStruct can only be created from named fields",
            ));
        };

        if attrs.wraps && fields.named.len() == 1 {
            let f = fields.named.first().unwrap();
            return Ok(NamedWrapperStruct {
                input,
                combined_generics,
                attrs,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            });
        }

        if let Some(field_attrs) = &attrs.fields {
            for (f, f_attr) in fields.named.iter().zip(field_attrs.iter()) {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return Ok(NamedWrapperStruct {
                            input,
                            combined_generics,
                            attrs,
                            wraps: f.ident.clone().unwrap(),
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        });
                    }
                }
            }
        }
        Err(syn::Error::new_spanned(
            input,
            "Struct is marked as wrapper but no field marked as wrapper or no single field present",
        ))
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        self.input
    }

    pub fn type_lattice(&self) -> Option<&syn::Type> {
        self.attrs.type_lattice.as_ref()
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for NamedWrapperStruct<'input, T>
where
    T: HasGenerics,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.generics().split_for_impl();
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
    T: HasDefaultCratePath,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}

pub struct UnnamedWrapperStruct<'input, T> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: StructAttribute,
    pub wraps: usize,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T: CombineGenerics> UnnamedWrapperStruct<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => StructAttribute::new(input)?,
        };
        let combined_generics = trait_info.combine_generics(&input.generics);

        let syn::Data::Struct(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "UnnamedWrapperStruct can only be created from struct data",
            ));
        };

        let syn::Fields::Unnamed(fields) = &data.fields else {
            return Err(syn::Error::new_spanned(
                input,
                "UnnamedWrapperStruct can only be created from unnamed fields",
            ));
        };

        if attrs.wraps && fields.unnamed.len() == 1 {
            let f = fields.unnamed.iter().next().unwrap();
            return Ok(Self {
                input,
                combined_generics,
                attrs,
                wraps: 0,
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            });
        }

        if let Some(field_attrs) = &attrs.fields {
            for (i, (f, f_attr)) in fields.unnamed.iter().zip(field_attrs.iter()).enumerate() {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return Ok(Self {
                            input,
                            combined_generics,
                            attrs,
                            wraps: i,
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        });
                    }
                }
            }
        }
        Err(syn::Error::new_spanned(
            input,
            "Struct is marked as wrapper but no field marked as wrapper or no single field present",
        ))
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        self.input
    }

    pub fn type_lattice(&self) -> Option<&syn::Type> {
        self.attrs.type_lattice.as_ref()
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for UnnamedWrapperStruct<'input, T>
where
    T: HasGenerics,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.generics().split_for_impl();
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
    T: HasDefaultCratePath,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}

impl<'input, T> std::fmt::Debug for WrapperStruct<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapperStruct::Named(data) => f
                .debug_struct("NamedWrapperStruct")
                .field("wraps", &data.wraps)
                .finish(),
            WrapperStruct::Unnamed(data) => f
                .debug_struct("UnnamedWrapperStruct")
                .field("wraps", &data.wraps)
                .finish(),
        }
    }
}
