use crate::field::iter::context::{DeriveFieldIter, StatementInfo};
use crate::field::iter::helpers::FieldInputBuilder;
use crate::field::iter::statement::StatementBuilder;
use kirin_derive_core::prelude::*;

impl<'ir> Scan<'ir, StandardLayout> for DeriveFieldIter {
    fn scan_input(&mut self, input: &'ir ir::Input<StandardLayout>) -> darling::Result<()> {
        self.input = Some(InputMeta::from_input(input));
        self.statements.clear();
        scan::scan_input(self, input)
    }

    fn scan_statement(
        &mut self,
        statement: &'ir ir::Statement<StandardLayout>,
    ) -> darling::Result<()> {
        let input = self.input_ctx()?;
        let input_builder = FieldInputBuilder::new(self, input);
        let builder = StatementBuilder::new(self, input);
        let pattern = builder.statement_pattern(statement);
        let pattern_empty = pattern.is_empty();
        let matching_item = input_builder.matching_item();
        let is_wrapper = statement.wraps.is_some();
        let (iter_expr, inner_type) = if is_wrapper {
            (
                builder.statement_wrapper_expr(statement),
                builder.statement_wrapper_type(statement),
            )
        } else {
            (
                builder.statement_iter_expr(statement, &matching_item),
                builder.statement_iter_type(statement, &matching_item),
            )
        };
        let info = StatementInfo {
            name: statement.name.clone(),
            pattern,
            pattern_empty,
            iter_expr,
            inner_type,
            is_wrapper,
        };
        self.statements.insert(statement.name.to_string(), info);
        Ok(())
    }
}
