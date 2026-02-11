use crate::property::context::{DeriveProperty, InputContext, PropertyKind};
use crate::property::statement::{StatementBuilder, StatementInfo};
use kirin_derive_core::derive::InputMeta as CoreInputMeta;
use kirin_derive_core::prelude::*;

impl<'ir> Scan<'ir, StandardLayout> for DeriveProperty {
    fn scan_input(&mut self, input: &'ir ir::Input<StandardLayout>) -> darling::Result<()> {
        self.input = Some(InputContext {
            core: CoreInputMeta::from_input(input),
            global_value: self.kind.global_value(input),
        });
        self.statements.clear();
        self.validate_speculatable_pure_invariant(input)?;
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
        self.statements.insert(statement.name.to_string(), info);
        Ok(())
    }
}

impl DeriveProperty {
    fn validate_speculatable_pure_invariant(
        &self,
        input: &ir::Input<StandardLayout>,
    ) -> darling::Result<()> {
        if !matches!(self.kind, PropertyKind::Speculatable) {
            return Ok(());
        }

        let mut errors = darling::Error::accumulator();
        let global_speculatable = input.attrs.speculatable;
        let global_pure = input.attrs.pure;

        match &input.data {
            ir::Data::Struct(statement) => {
                if statement.wraps.is_none() && global_speculatable && !global_pure {
                    errors.push(
                        darling::Error::custom(
                            "effective #[kirin(speculatable)] requires #[kirin(pure)]",
                        )
                        .with_span(&input.name),
                    );
                }
            }
            ir::Data::Enum(data) => {
                for statement in data.iter() {
                    if statement.wraps.is_some() {
                        continue;
                    }
                    let effective_speculatable =
                        global_speculatable || statement.attrs.speculatable;
                    let effective_pure = global_pure || statement.attrs.pure;
                    if effective_speculatable && !effective_pure {
                        errors.push(
                            darling::Error::custom(format!(
                                "variant '{}' is effectively #[kirin(speculatable)] but not #[kirin(pure)]",
                                statement.name
                            ))
                            .with_span(&statement.name),
                        );
                    }
                }
            }
        }

        errors.finish()
    }
}
