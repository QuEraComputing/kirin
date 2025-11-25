use crate::utils::{is_type, is_type_in_generic, is_vec_type};

pub enum NamedMatchingField {
    One(syn::Ident),
    Vec(syn::Ident),
}

pub enum UnnamedMatchingField {
    One(usize),
    Vec(usize),
}

impl NamedMatchingField {
    pub fn try_from_field(f: &syn::Field, matching_type: &syn::Ident) -> syn::Result<Option<Self>> {
        if is_type(&f.ty, matching_type) {
            Ok(Some(NamedMatchingField::One(f.ident.clone().ok_or_else(
                || syn::Error::new_spanned(f, "Expected named field to have an ident"),
            )?)))
        } else if is_vec_type(&f.ty, matching_type) {
            Ok(Some(NamedMatchingField::Vec(f.ident.clone().ok_or_else(
                || syn::Error::new_spanned(f, "Expected named field to have an ident"),
            )?)))
        } else if is_type_in_generic(&f.ty, matching_type) {
            Err(syn::Error::new_spanned(
                f,
                "generic types other than Vec are not supported",
            ))
        } else {
            Ok(None)
        }
    }
}

impl UnnamedMatchingField {
    pub fn try_from_field(
        index: usize,
        f: &syn::Field,
        matching_type: &syn::Ident,
    ) -> syn::Result<Option<Self>> {
        if is_type(&f.ty, matching_type) {
            Ok(Some(UnnamedMatchingField::One(index)))
        } else if is_vec_type(&f.ty, matching_type) {
            Ok(Some(UnnamedMatchingField::Vec(index)))
        } else if is_type_in_generic(&f.ty, matching_type) {
            Err(syn::Error::new_spanned(
                f,
                "generic types other than Vec are not supported",
            ))
        } else {
            Ok(None)
        }
    }
}
