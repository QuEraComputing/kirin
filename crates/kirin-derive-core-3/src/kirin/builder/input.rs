use super::context::Builder;
use crate::prelude::*;
use quote::quote;

target! {
    pub struct InputSignature
}

impl<'src> Compile<'src, Field<'_, 'src, Self>, InputSignature> for Builder {
    fn compile(&self, node: &Field<'_, 'src, Self>) -> InputSignature {
        let ty = &node.source().ty;
        if node.attrs().into {
            return quote! { #node: impl Into<#ty> }.into();
        } else {
            return quote! { #node: #ty }.into();
        }
    }
}

target! {
    /// let <name>: <decl ty> = <value>
    pub struct LetNameEqInput
}

impl<'src> Compile<'src, Field<'_, 'src, Self>, LetNameEqInput> for Builder {
    fn compile(&self, node: &Field<'_, 'src, Self>) -> LetNameEqInput {
        let name = &node.source_ident();
        let ty = &node.source().ty;
        if node.attrs().into {
            quote! {
                let #name: #ty = #name.into();
            }
        } else if let Some(default) = &node.attrs().default {
            quote! {
                let #name: #ty = #default;
            }
        } else if is_type(&node.source().ty, "PhantomData") {
            syn::Error::new_spanned(
                &node.source().ty,
                "use `#[kirin(default = std::marker::PhantomData)]` \
                            to initialize PhantomData fields",
            )
            .to_compile_error()
        } else {
            // no need to assign, nothing is created
            quote! {}
        }
        .into()
    }
}
