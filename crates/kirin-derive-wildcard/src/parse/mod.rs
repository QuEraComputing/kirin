//! Parsing a `#[rewrite_rule]` function into a [`RuleModel`](crate::model::RuleModel).

mod names;
mod rule_parser;
mod statements;

#[cfg(test)]
mod tests;

pub(crate) use rule_parser::RuleParser;
