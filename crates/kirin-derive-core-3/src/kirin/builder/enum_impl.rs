use quote::quote;

use crate::{
    kirin::builder::{build_fn::BuildFnImpl, build_result::BuildResultModule, from::FromImpl},
    prelude::*,
};

use super::context::Builder;

target! {
    pub struct EnumImpl
}

impl<'src> Compile<'src, Enum<'src, Builder>, EnumImpl> for Builder {
    fn compile(&self, node: &Enum<'src, Builder>) -> EnumImpl {
        if node.attrs().builder.is_none() {
            return quote! {}.into();
        }

        let build_result_mod: BuildResultModule = self.compile(node);
        let build_fn: BuildFnImpl = self.compile(node);
        let from_impls: Vec<FromImpl> = node
            .variants()
            .filter_map(|v| {
                if v.is_wrapper() {
                    Some(self.compile(&v.fields()))
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
