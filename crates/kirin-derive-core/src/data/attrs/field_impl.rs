use crate::data::attrs::utils::{error_unknown_attribute, parse_kirin_attributes};

use super::builder::FieldBuilder;

#[derive(Clone, Default)]
pub struct FieldAttribute {
    /// whether the field wraps another instruction
    pub wraps: bool,
    /// field builder options
    pub builder: Option<FieldBuilder>,
}

impl FieldAttribute {
    pub fn from_field_attrs(attrs: &Vec<syn::Attribute>) -> Option<Self> {
        if !attrs.iter().any(|attr| attr.path().is_ident("kirin")) {
            return None;
        }

        let mut field_attr = FieldAttribute::default();
        parse_kirin_attributes(attrs, |meta| {
            if meta.path.is_ident("wraps") {
                field_attr.wraps = true;
            } else if meta.path.is_ident("into") {
                field_attr.builder.get_or_insert_with(Default::default).into = true;
            } else if meta.path.is_ident("init") {
                let expr: syn::Expr = meta.value()?.parse()?;
                field_attr.builder.get_or_insert_with(Default::default).init = Some(expr);
            } else if meta.path.is_ident("type") {
                let expr: syn::Expr = meta.value()?.parse()?;
                field_attr.builder.get_or_insert_with(Default::default).ty = Some(expr);
            } else {
                return Err(error_unknown_attribute(&meta));
            }
            Ok(())
        })
        .unwrap();
        Some(field_attr)
    }
}
