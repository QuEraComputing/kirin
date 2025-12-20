use super::context::Builder;
use crate::prelude::*;
use quote::quote;

target! {
    pub struct Initialization
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, Initialization> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> Initialization {
        let names = node.iter().map(|f| f.source_ident()).collect::<Vec<_>>();

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
