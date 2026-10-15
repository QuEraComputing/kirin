use proc_macro2::Span;
use syn::parse_quote;

use crate::parse::RuleParser;

use super::helpers::{binding, compute, error, matched_parser, names, parser};

/// A parser whose match holds the statements `stmts`, in order.
fn parser_matching(stmts: &[syn::Stmt]) -> RuleParser {
    let mut parser = parser();
    for stmt in stmts {
        for id in parser.parse_statement(stmt).unwrap() {
            parser.push_body(id);
        }
    }
    parser
}

// --- resolve ---

#[test]
fn resolve_creates_one_hole_per_name() {
    let mut parser = parser();
    let x = parser.resolve_value(&parse_quote!(x)).unwrap();
    let x_again = parser.resolve_value(&parse_quote!(x)).unwrap();
    let y = parser.resolve_value(&parse_quote!(y)).unwrap();
    assert_eq!(x, x_again);
    assert_ne!(x, y);

    let model = parser.model;
    assert!(model.is_hole(x));
    assert_eq!(names(&model, &[x, y]), ["x", "y"]);
}

#[test]
fn resolve_returns_the_let_binding_of_a_bound_name() {
    let mut parser = parser();
    let stmt = parser
        .add_statement(Span::call_site(), compute(), vec![binding("r")])
        .unwrap();
    let r = parser.resolve_value(&parse_quote!(r)).unwrap();

    let model = parser.model;
    assert_eq!(model.statement(stmt).results, [r]);
    assert!(!model.is_hole(r));
}

#[test]
fn resolve_rejects_the_name_of_an_op() {
    let mut parser = parser_matching(&[parse_quote!(let s @ r = op![Arith::neg(x)];)]);
    assert_eq!(
        error(parser.resolve_value(&parse_quote!(s))),
        "`s` names an op, not a value: use one of its results"
    );
}

/// Resolving only says what a name means: a value already redirected, or a
/// result of an erased op, still resolves. `RuleModel::verify` reports such
/// a use.
#[test]
fn resolve_does_not_check_whether_a_value_is_still_usable() {
    let mut parser = matched_parser();
    let r = parser.resolve_value(&parse_quote!(r)).unwrap();
    parser
        .parse_rewrite_action(&parse_quote!(replace!(r, x)))
        .unwrap();
    parser
        .parse_rewrite_action(&parse_quote!(erase!(r)))
        .unwrap();
    assert_eq!(parser.resolve_value(&parse_quote!(r)).unwrap(), r);
}

// --- resolve_op ---

#[test]
fn resolve_op_takes_the_name_of_an_op_or_of_one_of_its_results() {
    let parser = parser_matching(&[parse_quote!(let dm @ (q, rem) = op![Arith::divmod(x, y)];)]);
    let op = *parser.model().body().last().unwrap();
    for target in [parse_quote!(dm), parse_quote!(q), parse_quote!(rem)] {
        assert_eq!(parser.resolve_op(&target, "misuse").unwrap(), op);
    }
}

#[test]
fn resolve_op_names_an_op_without_results() {
    let parser = parser_matching(&[parse_quote!(let k @ () = op![Sink::sink(x)];)]);
    let op = *parser.model().body().last().unwrap();
    assert!(parser.model().statement(op).results.is_empty());
    assert_eq!(parser.resolve_op(&parse_quote!(k), "misuse").unwrap(), op);
}

#[test]
fn resolve_op_rejects_anything_but_a_matched_op() {
    let parser = parser_matching(&[
        parse_quote!(let r = op![Arith::neg(x)];),
        parse_quote!(let k = f(x);),
    ]);
    // A hole, a native result, an unbound name, and something not a name.
    for target in [
        parse_quote!(x),
        parse_quote!(k),
        parse_quote!(unbound),
        parse_quote!(_),
        parse_quote!(a::b),
    ] {
        assert_eq!(error(parser.resolve_op(&target, "misuse")), "misuse");
    }
}

// --- add_statement ---

#[test]
fn add_statement_binds_named_results_in_order() {
    let mut parser = parser();
    let stmt = parser
        .add_statement(
            Span::call_site(),
            compute(),
            vec![binding("q"), (None, Span::call_site()), binding("r")],
        )
        .unwrap();

    let model = parser.model;
    let results = &model.statement(stmt).results;
    assert_eq!(names(&model, results), ["q", "_", "r"]);
    for &id in results {
        assert_eq!(model.value(id).stmt, stmt);
    }
}

#[test]
fn add_statement_rejects_a_name_used_before_its_let() {
    let mut parser = parser();
    parser.resolve_value(&parse_quote!(x)).unwrap();
    assert_eq!(
        error(parser.add_statement(Span::call_site(), compute(), vec![binding("x")])),
        "`x` is used before the `let` that binds it"
    );
}

#[test]
fn add_statement_rejects_a_name_bound_by_an_earlier_let() {
    let mut parser = parser();
    parser
        .add_statement(Span::call_site(), compute(), vec![binding("x")])
        .unwrap();
    assert_eq!(
        error(parser.add_statement(Span::call_site(), compute(), vec![binding("x")])),
        "duplicate identifier `x` in `#[rewrite_rule]` function"
    );
}

#[test]
fn add_statement_rejects_a_name_bound_twice_in_one_pattern() {
    let mut parser = parser();
    assert_eq!(
        error(parser.add_statement(
            Span::call_site(),
            compute(),
            vec![binding("x"), binding("x")],
        )),
        "duplicate identifier `x` in `#[rewrite_rule]` function"
    );
}

#[test]
fn add_named_statement_checks_the_op_name_like_the_results() {
    let duplicate = "duplicate identifier `s` in `#[rewrite_rule]` function";
    // The op and one of its results share a name.
    assert_eq!(
        error(parser().add_named_statement(
            Span::call_site(),
            compute(),
            Some(parse_quote!(s)),
            vec![binding("s")],
        )),
        duplicate
    );
    // The op takes a name an earlier `let` bound.
    let mut parser = parser();
    parser
        .add_statement(Span::call_site(), compute(), vec![binding("s")])
        .unwrap();
    assert_eq!(
        error(parser.add_named_statement(
            Span::call_site(),
            compute(),
            Some(parse_quote!(s)),
            vec![]
        )),
        duplicate
    );
}
