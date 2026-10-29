//! Generating the code a `#[rewrite_rule]` function expands into from its
//! [`RuleModel`](crate::model::RuleModel).

mod rule_codegen;
mod statements;

#[cfg(test)]
mod tests;

pub(crate) use rule_codegen::RuleCodegen;
