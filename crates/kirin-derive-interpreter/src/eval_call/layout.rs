use kirin_derive_toolkit::ir::Layout;
use kirin_derive_toolkit::prelude::darling::{self, FromDeriveInput, FromVariant};

#[derive(Debug, Clone)]
pub struct EvalCallLayout;

impl Layout for EvalCallLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = EvalCallGlobalAttrs;
    type ExtraStatementAttrs = EvalCallStatementAttrs;
    type ExtraFieldAttrs = ();

    fn extra_statement_attrs_from_input(
        input: &syn::DeriveInput,
    ) -> darling::Result<EvalCallStatementAttrs> {
        EvalCallStatementAttrs::from_derive_input(input)
    }
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
