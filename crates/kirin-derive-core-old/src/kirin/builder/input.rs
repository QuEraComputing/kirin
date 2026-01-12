use super::context::Builder;
use crate::{kirin::extra::FieldKind, prelude::*};
use quote::quote;

target! {
    pub struct InputSignature
}

impl<'src> Compile<'src, Builder, InputSignature> for Field<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> InputSignature {
        let ty = &self.source().ty;
        if !matches!(self.extra().kind, FieldKind::Other) || self.attrs().into {
            return quote! { #self: impl Into<#ty> }.into();
        } else {
            return quote! { #self: #ty }.into();
        }
    }
}

target! {
    /// let <name>: <decl ty> = <value>
    pub struct LetNameEqInput
}

impl<'src> Compile<'src, Builder, LetNameEqInput> for Field<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> LetNameEqInput {
        let name = &self.source_ident();
        let ty = &self.source().ty;
        if !matches!(self.extra().kind, FieldKind::Other) || self.attrs().into {
            quote! {
                let #name: #ty = #name.into();
            }
        } else if let Some(default) = &self.attrs().default {
            quote! {
                let #name: #ty = #default;
            }
        } else if is_type(&self.source().ty, "PhantomData") {
            syn::Error::new_spanned(
                &self.source().ty,
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
