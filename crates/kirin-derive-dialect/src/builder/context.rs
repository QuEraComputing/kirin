use crate::builder::statement::StatementInfo;
use crate::common;
use kirin_derive_core::derive::InputMeta;
use kirin_derive_core::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DeriveBuilder {
    pub default_crate_path: syn::Path,
    pub(crate) input: Option<InputMeta>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

impl Default for DeriveBuilder {
    fn default() -> Self {
        Self {
            default_crate_path: syn::parse_quote!(::kirin::ir),
            input: None,
            statements: HashMap::new(),
        }
    }
}

impl DeriveBuilder {
    pub fn new(default_crate_path: impl Into<String>) -> Self {
        Self {
            default_crate_path: syn::parse_str(&default_crate_path.into()).unwrap(),
            ..Self::default()
        }
    }

    pub fn emit(&mut self, input: &syn::DeriveInput) -> darling::Result<proc_macro2::TokenStream> {
        common::emit_from_derive_input(self, input)
    }

    pub fn emit_from_input(
        &mut self,
        input: &ir::Input<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        common::emit_from_ir(self, input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputMeta> {
        common::require_input_ctx(&self.input, "DeriveBuilder")
    }

    pub(crate) fn statement_info(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> darling::Result<&StatementInfo> {
        common::statement_info(&self.statements, statement)
    }

    pub(crate) fn full_crate_path(&self, input: &InputMeta) -> syn::Path {
        input
            .path_builder(&self.default_crate_path)
            .full_crate_path()
    }
}
