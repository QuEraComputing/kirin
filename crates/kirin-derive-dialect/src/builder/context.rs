use crate::builder::statement::StatementInfo;
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
        let input = ir::Input::<StandardLayout>::from_derive_input(input)?;
        self.scan_input(&input)?;
        self.emit_input(&input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputMeta> {
        self.input.as_ref().ok_or_else(|| {
            darling::Error::custom("DeriveBuilder context missing, call scan_input first")
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

    pub(crate) fn full_crate_path(&self, input: &InputMeta) -> syn::Path {
        input.path_builder(&self.default_crate_path).full_crate_path()
    }
}
