use super::context::{Builder, FieldKind};
use crate::prelude::*;
use quote::{format_ident, quote};

target! {
    /// signature of the builder function (without name and generics)
    pub struct Signature
}

impl<'src, N> Compile<'src, N, Signature> for Builder
where
    N: StructOrVariant<'src, Self>,
    <N as Attrs>::Output: WithUserCratePath,
{
    fn compile(&self, node: &N) -> Signature {
        let inputs: Vec<InputSignature> = node
            .fields()
            .iter()
            .filter(|f| matches!(&f.extra().kind, FieldKind::ResultValue))
            .map(|f| self.compile(&f))
            .collect();

        let crate_path: CratePath = self.compile(node);
        let ref_struct_name: RefStructName = self.compile(node);

        quote! {
            (context: &mut #crate_path::Context<Lang>, #(#inputs),*) -> #ref_struct_name
        }
        .into()
    }
}

target! {
    pub struct RefStructName
}

impl<'src, N> Compile<'src, N, RefStructName> for Builder
where
    N: SourceIdent,
{
    fn compile(&self, node: &N) -> RefStructName {
        let name = format_ident!("Build{}Ref", node.source_ident());
        quote! { #name }.into()
    }
}

target! {
    pub struct InputSignature
}

impl<'src, Attr> Compile<'src, Field<'_, 'src, Attr, Self>, InputSignature> for Builder {
    fn compile(&self, node: &Field<'_, 'src, Attr, Self>) -> InputSignature {
        let ty = &node.source().ty;
        if node.attrs().into {
            return quote! { #node: impl Into<#ty> }.into();
        } else {
            return quote! { #node: #ty }.into();
        }
    }
}
