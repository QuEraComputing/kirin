use crate::property::statement::StatementInfo;
use kirin_derive_core_2::derive::InputContext as CoreInputContext;
use kirin_derive_core_2::misc::from_str;
use kirin_derive_core_2::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub enum PropertyKind {
    Constant,
    Pure,
    Terminator,
}

impl PropertyKind {
    pub(crate) fn global_value(self, input: &ir::Input<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => input.attrs.constant,
            PropertyKind::Pure => input.attrs.pure,
            PropertyKind::Terminator => input.attrs.terminator,
        }
    }

    pub(crate) fn statement_value(self, statement: &ir::Statement<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => statement.attrs.constant,
            PropertyKind::Pure => statement.attrs.pure,
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
    pub(crate) core: CoreInputContext,
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
        let input = ir::Input::<StandardLayout>::from_derive_input(input)?;
        self.scan_input(&input)?;
        self.emit_input(&input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputContext> {
        self.input.as_ref().ok_or_else(|| {
            darling::Error::custom("DeriveProperty context missing, call scan_input first")
        })
    }

    pub(crate) fn statement_info(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> darling::Result<&StatementInfo> {
        let key = statement.name.to_string();
        self.statements.get(&key).ok_or_else(|| {
            darling::Error::custom(format!(
                "Missing statement info for '{}', call scan_statement first",
                key
            ))
        })
    }

    pub(crate) fn full_trait_path(&self, input: &InputContext) -> syn::Path {
        input
            .core
            .builder(&self.default_crate_path)
            .full_trait_path(&self.trait_path)
    }
}
