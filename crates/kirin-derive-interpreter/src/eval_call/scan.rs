use super::{DeriveEvalCall, EvalCallLayout, InputContext, StatementInfo};
use crate::pattern::build_pattern;
use kirin_derive_core::derive::InputMeta;
use kirin_derive_core::prelude::*;
use quote::quote;

impl<'ir> Scan<'ir, EvalCallLayout> for DeriveEvalCall {
    fn scan_input(&mut self, input: &'ir ir::Input<EvalCallLayout>) -> darling::Result<()> {
        self.input = Some(InputContext {
            core: InputMeta::from_input(input),
            callable_all: input.extra_attrs.callable,
        });
        self.statements.clear();
        scan::scan_input(self, input)
    }

    fn scan_statement(
        &mut self,
        statement: &'ir ir::Statement<EvalCallLayout>,
    ) -> darling::Result<()> {
        let is_wrapper = statement.wraps.is_some();
        let wrapper_ty = statement.wraps.as_ref().map(|w| w.ty.clone());
        let wrapper_binding = statement.wraps.as_ref().map(|w| {
            let name = w.field.name();
            quote! { #name }
        });

        let callable_all = self.input_ctx()?.callable_all;
        let is_callable = callable_all || statement.extra_attrs.callable;

        let pattern = build_pattern(statement);

        let info = StatementInfo {
            name: statement.name.clone(),
            is_wrapper,
            is_callable,
            wrapper_ty,
            wrapper_binding,
            pattern,
        };
        self.statements.insert(statement.name.to_string(), info);
        Ok(())
    }
}
