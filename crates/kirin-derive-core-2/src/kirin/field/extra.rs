use super::context::FieldsIter;
use crate::{
    data::*,
    utils::{is_type, is_type_in_generic, is_vec_type},
};

#[derive(Debug)]
pub enum FieldExtra {
    One,
    Vec,
    Other,
}

impl<'src> FromContext<'src, FieldsIter, syn::Field> for FieldExtra {
    fn from_context(ctx: &FieldsIter, node: &'src syn::Field) -> syn::Result<Self> {
        let matching_type = &ctx.matching_type_name;
        if is_type(&node.ty, matching_type) {
            Ok(FieldExtra::One)
        } else if is_vec_type(&node.ty, matching_type) {
            Ok(FieldExtra::Vec)
        } else if is_type_in_generic(&node.ty, matching_type) {
            Err(syn::Error::new_spanned(
                node,
                format!(
                    "Field type matches the matching type '{}' only in a generic context. \
                    Consider using 'Vec<{}>' or '{}' directly.",
                    matching_type, matching_type, matching_type
                ),
            ))
        } else {
            Ok(FieldExtra::Other)
        }
    }
}
