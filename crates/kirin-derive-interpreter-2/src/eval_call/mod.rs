mod emit;
mod layout;
mod scan;

pub use layout::EvalCallLayout;

use kirin_derive_toolkit::derive::InputMeta;
use kirin_derive_toolkit::emit::Emit;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::scan::Scan;
use kirin_derive_toolkit::tokens::Pattern;
use std::collections::HashMap;

pub struct DeriveEvalCall {
    pub(crate) default_interpreter_crate: syn::Path,
    pub(crate) default_ir_crate: syn::Path,
    pub(crate) input: Option<InputContext>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct InputContext {
    pub(crate) core: InputMeta,
    pub(crate) callable_all: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) is_wrapper: bool,
    pub(crate) is_callable: bool,
    pub(crate) wrapper_ty: Option<syn::Type>,
    pub(crate) wrapper_binding: Option<proc_macro2::TokenStream>,
    pub(crate) pattern: Pattern,
}

impl Default for DeriveEvalCall {
    fn default() -> Self {
        Self {
            default_interpreter_crate: from_str("::kirin_interpreter"),
            default_ir_crate: from_str("::kirin::ir"),
            input: None,
            statements: HashMap::new(),
        }
    }
}

impl DeriveEvalCall {
    pub fn emit(&mut self, input: &syn::DeriveInput) -> darling::Result<proc_macro2::TokenStream> {
        let input =
            kirin_derive_toolkit::ir::Input::<EvalCallLayout>::from_derive_input(input)?;
        self.scan_input(&input)?;
        self.emit_input(&input)
    }

    pub(crate) fn input_ctx(&self) -> darling::Result<&InputContext> {
        self.input.as_ref().ok_or_else(|| {
            darling::Error::custom("DeriveEvalCall context missing, call scan_input first")
        })
    }

    pub(crate) fn statement_info(
        &self,
        statement: &kirin_derive_toolkit::ir::Statement<EvalCallLayout>,
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

    pub(crate) fn ir_crate_path(&self, input: &InputContext) -> syn::Path {
        input
            .core
            .crate_path
            .clone()
            .unwrap_or_else(|| self.default_ir_crate.clone())
    }
}
