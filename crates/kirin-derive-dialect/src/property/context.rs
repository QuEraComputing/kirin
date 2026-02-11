use crate::common;
use crate::property::statement::StatementInfo;
use kirin_derive_core::derive::InputMeta as CoreInputMeta;
use kirin_derive_core::misc::from_str;
use kirin_derive_core::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub enum PropertyKind {
    Constant,
    Pure,
    Speculatable,
    Terminator,
}

impl PropertyKind {
    pub(crate) fn global_value(self, input: &ir::Input<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => input.attrs.constant,
            PropertyKind::Pure => input.attrs.pure,
            PropertyKind::Speculatable => input.attrs.speculatable,
            PropertyKind::Terminator => input.attrs.terminator,
        }
    }

    pub(crate) fn statement_value(self, statement: &ir::Statement<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => statement.attrs.constant,
            PropertyKind::Pure => statement.attrs.pure,
            PropertyKind::Speculatable => statement.attrs.speculatable,
            PropertyKind::Terminator => statement.attrs.terminator,
        }
    }
}

pub struct DeriveProperty {
    pub kind: PropertyKind,
    pub default_crate_path: syn::Path,
    pub trait_path: syn::Path,
    pub trait_method: syn::Ident,
    pub value_type: syn::Type,
    pub(crate) input: Option<InputContext>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct InputContext {
    pub(crate) core: CoreInputMeta,
    pub(crate) global_value: bool,
}

impl DeriveProperty {
    pub fn new(
        kind: PropertyKind,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            default_crate_path: from_str(default_crate_path),
            trait_path: from_str(trait_path),
            trait_method: from_str(trait_method),
            value_type: from_str(value_type),
            input: None,
            statements: HashMap::new(),
        }
    }

    pub fn emit(&mut self, input: &syn::DeriveInput) -> darling::Result<proc_macro2::TokenStream> {
        common::emit_from_derive_input(self, input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputContext> {
        common::require_input_ctx(&self.input, "DeriveProperty")
    }

    pub(crate) fn statement_info(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> darling::Result<&StatementInfo> {
        common::statement_info(&self.statements, statement)
    }

    pub(crate) fn full_trait_path(&self, input: &InputContext) -> syn::Path {
        input
            .core
            .path_builder(&self.default_crate_path)
            .full_trait_path(&self.trait_path)
    }
}
