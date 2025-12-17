use quote::quote;

use super::{name::Name, type_head::TypeGenerics};
use crate::{data::*, kirin::field::FieldsIter, target};

target! {
    /// impl head of trait `Iterator` for the field iterator type
    /// 
    /// ```ignore
    /// impl #impl_generics Iterator for #iter_name #ty_generics #where_clause
    /// ```
    pub struct ImplHead
}

impl<'src, T> Compile<'src, T, ImplHead> for FieldsIter
where
    T: Source<Output = syn::DeriveInput> + ContainsWrapper,
    Self: Compile<'src, T, TypeGenerics> + Compile<'src, T, Name>,
{
    fn compile(&self, node: &T) -> ImplHead {
        let iter_name: Name = self.compile(node);
        let generics: TypeGenerics = self.compile(node);
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        ImplHead(quote! {
            #[automatically_derived]
            impl #impl_generics Iterator for #iter_name #ty_generics #where_clause
        })
    }
}
