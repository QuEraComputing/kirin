use quote::quote;

use crate::{
    kirin::builder::{build_fn::BuildFnImpl, build_result::BuildResultModule, from::FromImpl},
    prelude::*,
};

use super::context::Builder;

target! {
    pub struct EnumImpl
}

impl<'src> Compile<'src, Builder, EnumImpl> for Enum<'src, Builder> {
    fn compile(&self, ctx: &Builder) -> EnumImpl {
        if self.attrs().builder.is_none() {
            return quote! {}.into();
        }

        let build_result_mod: BuildResultModule = self.compile(ctx);
        let build_fn: BuildFnImpl = self.compile(ctx);
        let from_impls: Vec<FromImpl> = self
            .variants()
            .filter_map(|v| {
                if v.is_wrapper() {
                    Some(v.fields().compile(ctx))
                } else {
                    None
                }
            })
            .collect();

        quote! {
            #build_fn
            #build_result_mod
            #(#from_impls)*
        }
        .into()
    }
}
