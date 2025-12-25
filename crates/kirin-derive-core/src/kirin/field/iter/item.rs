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

impl<'src, T> Compile<'src, FieldsIter, MatchingItem> for T
where
    T: WithUserCratePath,
{
    fn compile(&self, ctx: &FieldsIter) -> MatchingItem {
        let crate_path: CratePath = self.compile(ctx);
        let lifetime = &ctx.trait_lifetime;
        let matching_type = &ctx.matching_type;
        if ctx.mutable {
            MatchingItem(quote! { &#lifetime mut #crate_path :: #matching_type })
        } else {
            MatchingItem(quote! { &#lifetime #crate_path :: #matching_type })
        }
    }
}
