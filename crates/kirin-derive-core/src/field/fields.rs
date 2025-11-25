use crate::data::{FromStructFields, FromVariantFields};

use super::info::FieldIterInfo;
use super::named::NamedFields;
use super::unnamed::UnnamedFields;

pub enum Fields {
    Named(NamedFields),
    Unnamed(UnnamedFields),
    Unit,
}

impl Fields {
    pub fn new(fields: &syn::Fields, matching_type: &syn::Ident) -> syn::Result<Self> {
        match fields {
            syn::Fields::Named(named) => Ok(Fields::Named(NamedFields::new(named, matching_type)?)),
            syn::Fields::Unnamed(unnamed) => {
                Ok(Fields::Unnamed(UnnamedFields::new(unnamed, matching_type)?))
            }
            syn::Fields::Unit => Ok(Fields::Unit),
        }
    }
}

impl<'a> FromStructFields<'a, FieldIterInfo> for Fields {
    fn from_struct_fields(
        trait_info: &FieldIterInfo,
        _attrs: &crate::data::StructAttribute,
        _parent: &'a syn::DataStruct,
        fields: &'a syn::Fields,
    ) -> syn::Result<Self> {
        Self::new(fields, &trait_info.matching_type_name)
    }
}

impl<'a> FromVariantFields<'a, FieldIterInfo> for Fields {
    fn from_variant_fields(
        trait_info: &FieldIterInfo,
        _attrs: &crate::data::VariantAttribute,
        _parent: &'a syn::Variant,
        fields: &'a syn::Fields,
    ) -> syn::Result<Self> {
        Self::new(fields, &trait_info.matching_type_name)
    }
}
