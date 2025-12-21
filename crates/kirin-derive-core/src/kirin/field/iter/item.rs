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

impl<'src, T> Compile<'src, T, MatchingItem> for FieldsIter
where
    T: WithUserCratePath,
{
    fn compile(&self, node: &T) -> MatchingItem {
        let crate_path: CratePath = self.compile(node);
        let lifetime = &self.trait_lifetime;
        let matching_type = &self.matching_type;
        if self.mutable {
            MatchingItem(quote! { &#lifetime mut #crate_path :: #matching_type })
        } else {
            MatchingItem(quote! { &#lifetime #crate_path :: #matching_type })
        }
    }
}
