use super::context::{Property, SearchProperty};
use crate::prelude::*;
use quote::{ToTokens, quote};

target! {
    pub struct EnumImpl
}

impl<'src, S: SearchProperty> Compile<'src, Property<S>, EnumImpl> for Enum<'src, Property<S>> {
    fn compile(&self, ctx: &Property<S>) -> EnumImpl {
        let value_type = &ctx.value_type;
        let variant_ident = self.variant_names();
        let unpacking = self.unpacking();
        let glob = S::search_enum(self);
        let action = self
            .variants()
            .map(|v| {
                if let Some(wrapper) = v.wrapper() {
                    let wrapper_type = &wrapper.source().ty;
                    let trait_path = &ctx.trait_path;
                    let trait_method = &ctx.trait_method;
                    quote! {
                        <#wrapper_type as #trait_path>::#trait_method(#wrapper)
                    }
                } else {
                    let value = S::search_variant(&v);
                    let combined = S::combine(&glob, &value);
                    combined
                }
            })
            .collect::<Vec<_>>();

        let trait_path: TraitPath = self.compile(ctx);
        let trait_impl = TraitImpl::default()
            .input(self.source())
            .trait_path(trait_path)
            .add_method(
                TraitItemFnImpl::new(&ctx.trait_method)
                    .with_output(quote! {#value_type})
                    .with_token_body(quote! {
                        match self {
                            #(
                                Self::#variant_ident #unpacking => {
                                    #action
                                }
                            ),*
                        }
                    }),
            );
        trait_impl.to_token_stream().into()
    }
}
