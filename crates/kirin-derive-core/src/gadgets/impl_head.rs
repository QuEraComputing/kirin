use quote::quote;

use crate::{
    derive::Compile,
    ir::{Input, Layout, Source, SourceIdent},
    target,
};

target! {
    pub struct ImplHead
}

impl<L: Layout> Compile<'_, Input<'_, L>, ImplHead> for L {
    fn compile(&self, node: &Input<'_, L>) -> ImplHead {
        let source_ident = node.source_ident();
        let (impl_generics, ty_generics, where_clause) = node.source().generics.split_for_impl();
        quote! {
            impl #impl_generics #source_ident #ty_generics #where_clause
        }
        .into()
    }
}
