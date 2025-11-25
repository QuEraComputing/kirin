use quote::ToTokens;

use crate::data::VariantAttribute;

pub enum WrapperVariant<'input, T> {
    Named(NamedWrapperVariant<'input, T>),
    Unnamed(UnnamedWrapperVariant<'input, T>),
}

#[bon::bon]
impl<'input, T> WrapperVariant<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> syn::Result<Self> {
        match &variant.fields {
            syn::Fields::Named(_) => Ok(Self::Named(
                NamedWrapperVariant::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .variant(variant)
                    .build()?,
            )),
            syn::Fields::Unnamed(_) => Ok(Self::Unnamed(
                UnnamedWrapperVariant::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(attrs)
                    .variant(variant)
                    .build()?,
            )),
            _ => Err(syn::Error::new_spanned(
                variant,
                "WrapperVariant can only be created from named or unnamed fields",
            )),
        }
    }

    pub fn wraps_type(&self) -> &syn::Type {
        match self {
            WrapperVariant::Named(data) => &data.wraps_type,
            WrapperVariant::Unnamed(data) => &data.wraps_type,
        }
    }
}

impl<'input, T> std::fmt::Debug for WrapperVariant<'input, T> {
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

pub struct NamedWrapperVariant<'input, T> {
    pub variant: &'input syn::Variant,
    pub attrs: VariantAttribute,
    pub variant_name: &'input syn::Ident,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T> NamedWrapperVariant<'input, T> {
    #[builder]
    pub fn new(
        _trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => VariantAttribute::new(variant)?,
        };

        let syn::Fields::Named(fields) = &variant.fields else {
            return Err(syn::Error::new_spanned(
                variant,
                "NamedWrapperVariant can only be created from named fields",
            ));
        };

        if fields.named.len() == 1 {
            let f = fields
                .named
                .first()
                .ok_or_else(|| syn::Error::new_spanned(variant, "Expected one named field"))?;
            return Ok(NamedWrapperVariant {
                variant,
                attrs,
                variant_name: &variant.ident,
                wraps: f.ident.clone().ok_or_else(|| {
                    syn::Error::new_spanned(f.ident.clone(), "Expected one named field")
                })?,
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            });
        }

        if let Some(field_attrs) = &attrs.fields {
            for (f, f_attr) in fields.named.iter().zip(field_attrs.iter()) {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return Ok(NamedWrapperVariant {
                            variant,
                            attrs,
                            variant_name: &variant.ident,
                            wraps: f.ident.clone().ok_or_else(|| {
                                syn::Error::new_spanned(
                                    f.ident.clone(),
                                    "Expected named field to have an ident",
                                )
                            })?,
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        });
                    }
                }
            }
        }
        return Err(syn::Error::new_spanned(
            variant,
            "Variant marked as wrapper but no field marked as wrapper or no single field present",
        ));
    }
}

impl<'input, T> std::fmt::Debug for NamedWrapperVariant<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedWrapperVariant")
            .field("variant_name", &self.variant_name)
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
    }
}

pub struct UnnamedWrapperVariant<'input, T> {
    pub variant: &'input syn::Variant,
    pub attrs: VariantAttribute,
    pub variant_name: &'input syn::Ident,
    pub wraps: usize,
    pub wraps_type: syn::Type,
    _marker: std::marker::PhantomData<T>,
}

#[bon::bon]
impl<'input, T> UnnamedWrapperVariant<'input, T> {
    #[builder]
    pub fn new(
        _trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => VariantAttribute::new(variant)?,
        };
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            return Err(syn::Error::new_spanned(
                variant,
                "UnnamedWrapperVariant can only be created from unnamed fields",
            ));
        };
        if fields.unnamed.len() == 1 {
            let f = fields
                .unnamed
                .first()
                .ok_or_else(|| syn::Error::new_spanned(variant, "Expected one unnamed field"))?;
            return Ok(UnnamedWrapperVariant {
                variant,
                attrs,
                variant_name: &variant.ident,
                wraps: 0,
                wraps_type: f.ty.clone(),
                _marker: std::marker::PhantomData,
            });
        }

        if let Some(field_attrs) = &attrs.fields {
            for (index, (f, f_attr)) in fields.unnamed.iter().zip(field_attrs.iter()).enumerate() {
                if let Some(f_attr) = f_attr {
                    if f_attr.wraps {
                        return Ok(UnnamedWrapperVariant {
                            variant,
                            attrs,
                            variant_name: &variant.ident,
                            wraps: index,
                            wraps_type: f.ty.clone(),
                            _marker: std::marker::PhantomData,
                        });
                    }
                }
            }
        }
        return Err(syn::Error::new_spanned(
            variant,
            "Variant marked as wrapper but no field marked as wrapper or no single field present",
        ));
    }
}

impl<'input, T> std::fmt::Debug for UnnamedWrapperVariant<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnnamedWrapperVariant")
            .field("variant_name", &self.variant_name)
            .field("wraps", &self.wraps)
            .field("wraps_type", &self.wraps_type.to_token_stream())
            .finish()
    }
}
