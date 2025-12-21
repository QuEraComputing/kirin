use crate::prelude::*;

use super::context::FieldsIter;

#[derive(Debug)]
pub enum FieldExtra {
    One,
    Vec,
    Other,
}

impl<'src> ScanExtra<'src, syn::Field, FieldExtra> for FieldsIter {
    fn scan_extra(&self, node: &'src syn::Field) -> syn::Result<FieldExtra> {
        let matching_type = &self.matching_type_name;
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
