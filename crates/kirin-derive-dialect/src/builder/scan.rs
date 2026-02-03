use crate::builder::context::DeriveBuilder;
use crate::builder::helpers::build_fn_name;
use crate::builder::statement::StatementInfo;
use kirin_derive_core::derive::InputMeta;
use kirin_derive_core::prelude::*;

impl<'ir> Scan<'ir, StandardLayout> for DeriveBuilder {
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
        let fields = statement.collect_fields();
        let build_fn_name = build_fn_name(input.is_enum, statement);
        let info = StatementInfo {
            name: statement.name.clone(),
            fields,
            build_fn_name,
            is_wrapper: statement.wraps.is_some(),
            wrapper_type: statement.wraps.as_ref().map(|w| w.ty.clone()),
        };
        self.statements.insert(statement.name.to_string(), info);
        Ok(())
    }
}
