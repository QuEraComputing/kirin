use crate::property::context::{DeriveProperty, InputContext};
use crate::property::statement::{StatementBuilder, StatementInfo};
use kirin_derive_core_2::derive::InputContext as CoreInputContext;
use kirin_derive_core_2::prelude::*;

impl<'ir> Scan<'ir, StandardLayout> for DeriveProperty {
    fn scan_input(&mut self, input: &'ir ir::Input<StandardLayout>) -> darling::Result<()> {
        self.input = Some(InputContext {
            core: CoreInputContext::from_input(input),
            global_value: self.kind.global_value(input),
        });
        self.statements.clear();
        scan::scan_input(self, input)
    }

    fn scan_statement(
        &mut self,
        statement: &'ir ir::Statement<StandardLayout>,
    ) -> darling::Result<()> {
        let input = self.input_ctx()?;
        let builder = StatementBuilder::new();
        let pattern = builder.statement_pattern(statement);
        let pattern_empty = pattern.is_empty();
        let value_expr = builder.statement_value_expr(self, input, statement);

        let info = StatementInfo {
            name: statement.name.clone(),
            pattern,
            pattern_empty,
            value_expr,
            is_wrapper: statement.wraps.is_some(),
        };
        self.statements
            .insert(statement.name.to_string(), info);
        Ok(())
    }
}
