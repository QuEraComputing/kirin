use quote::quote;

use super::{name::Name, type_head::TypeGenerics};
use crate::{kirin::field::context::FieldsIter, prelude::*};

target! {
    /// impl head of trait `Iterator` for the field iterator type
    ///
    /// ```ignore
    /// impl #impl_generics Iterator for #iter_name #ty_generics #where_clause
    /// ```
    pub struct ImplHead
}

impl<'src, T> Compile<'src, FieldsIter, ImplHead> for T
where
    T: Source<Output = &'src syn::DeriveInput>
        + Compile<'src, FieldsIter, TypeGenerics>
        + Compile<'src, FieldsIter, Name>,
{
    fn compile(&self, ctx: &FieldsIter) -> ImplHead {
        let iter_name: Name = self.compile(ctx);
        let generics: TypeGenerics = self.compile(ctx);
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        ImplHead(quote! {
            #[automatically_derived]
            impl #impl_generics Iterator for #iter_name #ty_generics #where_clause
        })
    }
}
