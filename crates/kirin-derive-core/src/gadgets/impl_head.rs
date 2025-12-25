use quote::quote;

use crate::{
    derive::Compile,
    ir::{Input, Layout, Source, SourceIdent},
    target,
};

target! {
    pub struct ImplHead
}

impl<L: Layout> Compile<'_, L, ImplHead> for Input<'_, L> {
    fn compile(&self, _ctx: &L) -> ImplHead {
        let source_ident = self.source_ident();
        let (impl_generics, ty_generics, where_clause) = self.source().generics.split_for_impl();
        quote! {
            impl #impl_generics #source_ident #ty_generics #where_clause
        }
        .into()
    }
}
