use super::{context::Builder, result::ResultNames};
use crate::{kirin::extra::FieldKind, prelude::*};
use quote::quote;

target! {
    pub struct InitializationHead
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, InitializationHead> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> InitializationHead {
        match node.definition() {
            DefinitionStructOrVariant::Struct(..) => {
                quote! { Self }
            }
            DefinitionStructOrVariant::Variant(..) => {
                let name = node.source_ident();
                quote! { Self::#name }
            }
        }
        .into()
    }
}

target! {
    pub struct Initialization
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, Initialization> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> Initialization {
        let mut result_count: usize = 0;
        let result_names: ResultNames = self.compile(node);
        let names = node
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

        match node.source() {
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
