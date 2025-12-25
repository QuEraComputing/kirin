use super::{context::Builder, result::ResultNames};
use crate::{kirin::extra::FieldKind, prelude::*};
use quote::quote;

target! {
    pub struct InitializationHead
}

impl<'src> Compile<'src, Builder, InitializationHead> for Fields<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> InitializationHead {
        match self.definition() {
            DefinitionStructOrVariant::Struct(..) => {
                quote! { Self }
            }
            DefinitionStructOrVariant::Variant(..) => {
                let name = self.source_ident();
                quote! { Self::#name }
            }
        }
        .into()
    }
}

target! {
    pub struct Initialization
}

impl<'src> Compile<'src, Builder, Initialization> for Fields<'_, 'src, Builder> {
    fn compile(&self, ctx: &Builder) -> Initialization {
        let mut result_count: usize = 0;
        let result_names: ResultNames = self.compile(ctx);
        let names = self
            .iter()
            .map(|f| {
                if matches!(f.extra().kind, FieldKind::ResultValue) {
                    let name = &result_names.0.as_slice()[result_count];
                    result_count += 1;
                    name.clone()
                } else {
                    f.source_ident()
                }
            })
            .collect::<Vec<_>>();

        match self.source() {
            syn::Fields::Named(_) => {
                quote! { { #(#names,)* }  }
            }
            syn::Fields::Unnamed(_) => {
                quote! { ( #(#names,)* ) }
            }
            syn::Fields::Unit => {
                quote! {}
            }
        }
        .into()
    }
}
