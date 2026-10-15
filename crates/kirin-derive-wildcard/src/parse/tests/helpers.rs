//! Shared helpers for the parser's tests.

use proc_macro2::Span;
use syn::parse_quote;

use crate::model::{Binding, RuleModel, StmtId, StmtKind, Use, ValueId};
use crate::parse::RuleParser;

pub(super) fn parser() -> RuleParser {
    RuleParser::new(parse_quote!(rule))
}

/// A parser whose match already binds `r`, the result of
/// `op![Arith::neg(x)]`, so rewrite actions have a matched op to act on.
pub(super) fn matched_parser() -> RuleParser {
    let mut parser = parser();
    let ids = parser
        .parse_statement(&parse_quote!(let r = op![Arith::neg(x)];))
        .unwrap();
    for id in ids {
        parser.push_body(id);
    }
    parser
}

/// The message of the error `result` must hold.
pub(super) fn error<T: std::fmt::Debug>(result: syn::Result<T>) -> String {
    result.unwrap_err().to_string()
}

pub(super) fn local(stmt: syn::Stmt) -> syn::Local {
    let syn::Stmt::Local(local) = stmt else {
        panic!("expected a `let` statement");
    };
    local
}

/// The last statement of `block`: the only place a statement can omit its
/// semicolon.
pub(super) fn tail(block: syn::Block) -> syn::Stmt {
    block
        .stmts
        .into_iter()
        .last()
        .expect("expected a statement")
}

pub(super) fn pat_ident(pat: syn::Pat) -> syn::PatIdent {
    let syn::Pat::Ident(ident) = pat else {
        panic!("expected an identifier pattern");
    };
    ident
}

/// A result of a statement, as [`RuleParser::add_statement`] takes it.
pub(super) fn binding(name: &str) -> Binding {
    (
        Some(syn::Ident::new(name, Span::call_site())),
        Span::call_site(),
    )
}

pub(super) fn compute() -> StmtKind {
    StmtKind::NativeCompute {
        func: parse_quote!(f),
        args: vec![],
    }
}

/// The name of a value, or `_` for an anonymous one.
pub(super) fn name(model: &RuleModel, id: ValueId) -> String {
    model
        .value(id)
        .name
        .as_ref()
        .map_or_else(|| "_".to_string(), ToString::to_string)
}

pub(super) fn names(model: &RuleModel, ids: &[ValueId]) -> Vec<String> {
    ids.iter().map(|&id| name(model, id)).collect()
}

/// The values `uses` refer to, in order.
pub(super) fn ids(uses: &[Use]) -> Vec<ValueId> {
    uses.iter().map(|value| value.value).collect()
}

pub(super) fn kind_name(model: &RuleModel, id: StmtId) -> &'static str {
    match model.statement(id).kind {
        StmtKind::Hole => "hole",
        StmtKind::Op { .. } => "op",
        StmtKind::NativeCompute { .. } => "compute",
        StmtKind::Constraint { .. } => "constraint",
        StmtKind::Rewrite { .. } => "rewrite",
        StmtKind::Replace { .. } => "replace",
        StmtKind::Erase { .. } => "erase",
    }
}
