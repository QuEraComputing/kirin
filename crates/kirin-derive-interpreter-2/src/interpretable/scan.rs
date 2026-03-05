use super::{DeriveInterpretable, InputContext, StatementInfo};
use crate::pattern::build_pattern;
use kirin_derive_toolkit::derive::InputMeta;
use kirin_derive_toolkit::ir::StandardLayout;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::scan::{self, Scan};
use quote::quote;

impl<'ir> Scan<'ir, StandardLayout> for DeriveInterpretable {
    fn scan_input(
        &mut self,
        input: &'ir kirin_derive_toolkit::ir::Input<StandardLayout>,
    ) -> darling::Result<()> {
        self.input = Some(InputContext {
            core: InputMeta::from_input(input),
        });
        self.statements.clear();
        scan::scan_input(self, input)
    }

    fn scan_statement(
        &mut self,
        statement: &'ir kirin_derive_toolkit::ir::Statement<StandardLayout>,
    ) -> darling::Result<()> {
        let is_wrapper = statement.wraps.is_some();
        let wrapper_ty = statement.wraps.as_ref().map(|w| w.ty.clone());
        let wrapper_binding = statement.wraps.as_ref().map(|w| {
            let name = w.field.name();
            quote! { #name }
        });

        let pattern = build_pattern(statement);

        let info = StatementInfo {
            name: statement.name.clone(),
            is_wrapper,
            wrapper_ty,
            wrapper_binding,
            pattern,
        };
        self.statements.insert(statement.name.to_string(), info);
        Ok(())
    }
}
