mod emit;
mod scan;

use kirin_derive_core::derive::InputMeta;
use kirin_derive_core::misc::from_str;
use kirin_derive_core::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub struct DeriveInterpretable {
    pub(crate) default_interpreter_crate: syn::Path,
    pub(crate) input: Option<InputContext>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct InputContext {
    pub(crate) core: InputMeta,
}

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) is_wrapper: bool,
    pub(crate) wrapper_ty: Option<syn::Type>,
    pub(crate) wrapper_binding: Option<proc_macro2::TokenStream>,
    pub(crate) pattern: FieldPatternTokens,
}

impl Default for DeriveInterpretable {
    fn default() -> Self {
        Self {
            default_interpreter_crate: from_str("::kirin_interpreter"),
            input: None,
            statements: HashMap::new(),
        }
    }
}

impl DeriveInterpretable {
    pub fn emit(&mut self, input: &syn::DeriveInput) -> darling::Result<proc_macro2::TokenStream> {
        let input = ir::Input::<StandardLayout>::from_derive_input(input)?;
        self.scan_input(&input)?;
        self.emit_input(&input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputContext> {
        self.input.as_ref().ok_or_else(|| {
            darling::Error::custom("DeriveInterpretable context missing, call scan_input first")
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

    pub(crate) fn interpreter_crate_path(&self) -> syn::Path {
        self.default_interpreter_crate.clone()
    }
}
