use quote::quote;

use crate::{
    kirin::builder::{build_fn::BuildFnImpl, build_result::BuildResultModule, from::FromImpl},
    prelude::*,
};

use super::context::Builder;

target! {
    pub struct StructImpl
}

impl<'src> Compile<'src, Builder, StructImpl> for Struct<'src, Builder> {
    fn compile(&self, ctx: &Builder) -> StructImpl {
        if self.attrs().builder.is_none() {
            return quote! {}.into();
        }

        if self.is_wrapper() {
            let from_impl: FromImpl = self.fields().compile(ctx);
            return from_impl.to_token_stream().into();
        }

        let build_result_mod: BuildResultModule = self.compile(ctx);
        let build_fn: BuildFnImpl = self.compile(ctx);
        quote! {
            #build_fn
            #build_result_mod
        }
        .into()
    }
}
