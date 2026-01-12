use super::context::{Property, SearchProperty};
use crate::prelude::*;
use quote::{ToTokens, quote};

target! {
    pub struct StructImpl
}

impl<'src, S: SearchProperty> Compile<'src, Property<S>, StructImpl> for Struct<'src, Property<S>> {
    fn compile(&self, ctx: &Property<S>) -> StructImpl {
        let trait_method = &ctx.trait_method;
        let trait_path: TraitPath = self.compile(ctx);
        let trait_fn_impl = if let Some(wrapper) = &self.wrapper() {
            let wrapper_type = &wrapper.source().ty;
            let unpacking = self.unpacking();
            TraitItemFnImpl::new(&ctx.trait_method)
                .with_output(&ctx.value_type)
                .with_token_body(quote! {
                    let Self #unpacking = self;
                    <#wrapper_type as #trait_path>::#trait_method(#wrapper)
                })
        } else {
            let value = S::search_struct(self);
            TraitItemFnImpl::new(&ctx.trait_method)
                .with_output(&ctx.value_type)
                .with_token_body(quote! {
                    #value
                })
        };
        TraitImpl::default()
            .input(self.source())
            .trait_path(&ctx.trait_path)
            .add_method(trait_fn_impl)
            .to_token_stream()
            .into()
    }
}
