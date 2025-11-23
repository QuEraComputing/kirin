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
    pub fn try_from_field(f: &syn::Field, matching_type: &syn::Ident) -> Option<Self> {
        if is_type(&f.ty, matching_type) {
            Some(NamedMatchingField::One(f.ident.clone().unwrap()))
        } else if is_vec_type(&f.ty, matching_type) {
            Some(NamedMatchingField::Vec(f.ident.clone().unwrap()))
        } else if is_type_in_generic(&f.ty, matching_type) {
            panic!("generic types other than Vec are not supported");
        } else {
            None
        }
    }
}

impl UnnamedMatchingField {
    pub fn try_from_field(index: usize, f: &syn::Field, matching_type: &syn::Ident) -> Option<Self> {
        if is_type(&f.ty, matching_type) {
            Some(UnnamedMatchingField::One(index))
        } else if is_vec_type(&f.ty, matching_type) {
            Some(UnnamedMatchingField::Vec(index))
        } else if is_type_in_generic(&f.ty, matching_type) {
            panic!("generic types other than Vec are not supported");
        } else {
            None
        }
    }
}
