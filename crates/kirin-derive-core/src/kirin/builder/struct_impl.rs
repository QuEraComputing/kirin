use quote::quote;

use crate::{
    kirin::builder::{build_fn::BuildFnImpl, build_result::BuildResultModule, from::FromImpl},
    prelude::*,
};

use super::context::Builder;

target! {
    pub struct StructImpl
}

impl<'src> Compile<'src, Struct<'src, Self>, StructImpl> for Builder {
    fn compile(&self, node: &Struct<'src, Self>) -> StructImpl {
        if node.attrs().builder.is_none() {
            return quote! {}.into();
        }

        if node.is_wrapper() {
            let from_impl: FromImpl = self.compile(&node.fields());
            return from_impl.to_token_stream().into();
        }

        let build_result_mod: BuildResultModule = self.compile(node);
        let build_fn: BuildFnImpl = self.compile(node);
        quote! {
            #build_fn
            #build_result_mod
        }
        .into()
    }
}
