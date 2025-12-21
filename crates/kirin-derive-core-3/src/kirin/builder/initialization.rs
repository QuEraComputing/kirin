use super::context::Builder;
use crate::{kirin::extra::FieldKind, prelude::*};
use quote::{format_ident, quote};

target! {
    pub struct InitializationHead
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, InitializationHead> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> InitializationHead {
        match node.definition() {
            DefinitionStructOrVariant::Struct(_) => {
                quote! { Self }
            }
            DefinitionStructOrVariant::Variant(_) => {
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
        let mut result_count = 0;
        let names = node
            .iter()
            .map(|f| {
                if matches!(f.extra().kind, FieldKind::ResultValue) {
                    f.source().ident.clone().unwrap_or_else(|| {
                        let idx = result_count.to_string();
                        result_count += 1;
                        format_ident!("result_{}", idx, span = f.source_ident().span())
                    })
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
