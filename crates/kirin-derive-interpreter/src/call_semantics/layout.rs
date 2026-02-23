use kirin_derive_core::ir::Layout;
use kirin_derive_core::prelude::darling::{self, FromDeriveInput, FromVariant};

#[derive(Debug, Clone)]
pub struct CallSemanticsLayout;

impl Layout for CallSemanticsLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = CallSemanticsGlobalAttrs;
    type ExtraStatementAttrs = CallSemanticsStatementAttrs;
    type ExtraFieldAttrs = ();
}

#[derive(Debug, Clone)]
pub struct CallSemanticsGlobalAttrs {
    pub callable: bool,
}

impl FromDeriveInput for CallSemanticsGlobalAttrs {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        Ok(Self {
            callable: input.attrs.iter().any(|a| a.path().is_ident("callable")),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CallSemanticsStatementAttrs {
    pub callable: bool,
}

impl FromDeriveInput for CallSemanticsStatementAttrs {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        Ok(Self {
            callable: input.attrs.iter().any(|a| a.path().is_ident("callable")),
        })
    }
}

impl FromVariant for CallSemanticsStatementAttrs {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        Ok(Self {
            callable: variant.attrs.iter().any(|a| a.path().is_ident("callable")),
        })
    }
}
