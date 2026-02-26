use kirin_derive_core::ir::Layout;
use kirin_derive_core::prelude::darling::{self, FromDeriveInput, FromVariant};

#[derive(Debug, Clone)]
pub struct EvalCallLayout;

impl Layout for EvalCallLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = EvalCallGlobalAttrs;
    type ExtraStatementAttrs = EvalCallStatementAttrs;
    type ExtraFieldAttrs = ();
}

#[derive(Debug, Clone)]
pub struct EvalCallGlobalAttrs {
    pub callable: bool,
}

impl FromDeriveInput for EvalCallGlobalAttrs {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        Ok(Self {
            callable: input.attrs.iter().any(|a| a.path().is_ident("callable")),
        })
    }
}

#[derive(Debug, Clone)]
pub struct EvalCallStatementAttrs {
    pub callable: bool,
}

impl FromDeriveInput for EvalCallStatementAttrs {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        Ok(Self {
            callable: input.attrs.iter().any(|a| a.path().is_ident("callable")),
        })
    }
}

impl FromVariant for EvalCallStatementAttrs {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        Ok(Self {
            callable: variant.attrs.iter().any(|a| a.path().is_ident("callable")),
        })
    }
}
