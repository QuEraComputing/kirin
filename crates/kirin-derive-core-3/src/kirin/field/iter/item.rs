use quote::quote;

use crate::{kirin::field::context::FieldsIter, prelude::*};

target! {
    /// Matching item type of the field iterator, e.g
    /// ```ignore
    /// &'a SSAValue
    /// &'a mut SSAValue
    /// ```
    pub struct MatchingItem
}

impl<'src, T> Compile<'src, T, MatchingItem> for FieldsIter {
    fn compile(&self, _node: &T) -> MatchingItem {
        let lifetime = &self.trait_lifetime;
        let matching_type = self.absolute_crate_path(&self.matching_type);
        if self.mutable {
            MatchingItem(quote! { &#lifetime mut #matching_type })
        } else {
            MatchingItem(quote! { &#lifetime #matching_type })
        }
    }
}
