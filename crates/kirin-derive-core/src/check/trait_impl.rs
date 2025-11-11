use proc_macro2::TokenStream;

use crate::{check::{enum_impl::EnumChecker, struct_impl::StructChecker}, has_attr, is_attr_option_true};

pub struct CheckerInfo {
    /// The name of the item being checked.
    pub name: String,
    /// option name in `#kirin` attribute
    pub option: String,
    /// The path of the trait being checked.
    pub trait_path: syn::Path,
}

pub enum DataChecker<'input> {
    Struct(StructChecker<'input>),
    Enum(EnumChecker<'input>),
}

impl<'input> DataChecker<'input> {
    pub fn scan(checker: &'input CheckerInfo, input: &'input syn::DeriveInput) -> Self {
        match &input.data {
            syn::Data::Struct(data) => {
                DataChecker::Struct(StructChecker::scan(checker, input, data))
            }
            syn::Data::Enum(data) => {
                DataChecker::Enum(EnumChecker::scan(checker, input, data))
            }
            _ => panic!("only structs and enums are supported"),
        }
    }
}
