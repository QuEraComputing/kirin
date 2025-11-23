use quote::ToTokens;

use crate::data::{HasTraitGenerics, VariantAttribute};

pub enum WrapperVariant<'input, T: HasTraitGenerics> {
    Named(NamedWrapperVariant<'input, T>),
    Unnamed(UnnamedWrapperVariant<'input, T>),
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> WrapperVariant<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> Self {
        match &variant.fields {
            syn::Fields::Named(_) => Self::Named(
                NamedWrapperVariant::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .variant(variant)
                    .build(),
            ),
            syn::Fields::Unnamed(_) => Self::Unnamed(
                UnnamedWrapperVariant::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .variant(variant)
                    .build(),
            ),
            _ => panic!("WrapperVariant can only be created from named or unnamed fields"),
        }
    }

    pub fn wraps_type(&self) -> &syn::Type {
        match self {
            WrapperVariant::Named(data) => &data.wraps_type,
            WrapperVariant::Unnamed(data) => &data.wraps_type,
        }
    }
}

impl<'input, T: HasTraitGenerics> std::fmt::Debug for WrapperVariant<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapperVariant::Named(data) => {
                f.debug_tuple("WrapperVariant::Named").field(data).finish()
            }
            WrapperVariant::Unnamed(data) => f
                .debug_tuple("WrapperVariant::Unnamed")
                .field(data)
                .finish(),
        }
    }
}

pub struct NamedWrapperVariant<'input, T: HasTraitGenerics> {
    pub variant: &'input syn::Variant,
    pub attrs: VariantAttribute,
    pub variant_name: &'input syn::Ident,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> NamedWrapperVariant<'input, T> {
    #[builder]
    pub fn new(
        _trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| VariantAttribute::new(variant));

        let syn::Fields::Named(fields) = &variant.fields else {
            panic!("NamedWrapperVariant can only be created from named fields");
        };

        if fields.named.len() == 1 {
            let f = fields.named.first().unwrap();
            return NamedWrapperVariant {
                variant,
                attrs,
                variant_name: &variant.ident,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            };
        }

        if let Some(field_attrs) = &attrs.fields {
            for (f, f_attr) in fields.named.iter().zip(field_attrs.iter()) {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return NamedWrapperVariant {
                            variant,
                            attrs,
                            variant_name: &variant.ident,
                            wraps: f.ident.clone().unwrap(),
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        };
                    }
                }
            }
        }
        panic!("Variant {} marked as wrapper but could not be parsed as one", variant.ident);
    }
}

impl<'input, T: HasTraitGenerics> std::fmt::Debug for NamedWrapperVariant<'input, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedWrapperVariant")
            .field("variant_name", &self.variant_name)
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
    }
}

pub struct UnnamedWrapperVariant<'input, T: HasTraitGenerics> {
    pub variant: &'input syn::Variant,
    pub attrs: VariantAttribute,
    pub variant_name: &'input syn::Ident,
    pub wraps: usize,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> UnnamedWrapperVariant<'input, T> {
    #[builder]
    pub fn new(
        _trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| VariantAttribute::new(variant));
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            panic!("UnnamedWrapperVariant can only be created from unnamed fields");
        };
        if fields.unnamed.len() == 1 {
            let f = fields.unnamed.first().unwrap();
            return UnnamedWrapperVariant {
                variant,
                attrs,
                variant_name: &variant.ident,
                wraps: 0,
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            };
        }

        if let Some(field_attrs) = &attrs.fields {
            for (index, (f, f_attr)) in fields.unnamed.iter().zip(field_attrs.iter()).enumerate() {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return UnnamedWrapperVariant {
                            variant,
                            attrs,
                            variant_name: &variant.ident,
                            wraps: index,
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        };
                    }
                }
            }
        }
        panic!("Variant marked as wrapper but could not be parsed as one");
    }
}

impl<'input, T: HasTraitGenerics> std::fmt::Debug for UnnamedWrapperVariant<'input, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnnamedWrapperVariant")
            .field("variant_name", &self.variant_name)
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
    }
}
