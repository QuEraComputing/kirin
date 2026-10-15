use syn::parse_quote;

use crate::model::StmtKind;
use crate::parse::RuleParser;

use super::helpers::{
    error, ids, kind_name, local, matched_parser, name, names, parser, pat_ident, tail,
};

// --- parse_statement ---

#[test]
fn parse_statement_splits_an_unbound_call_into_a_call_and_a_constraint() {
    let mut parser = parser();
    let stmts = parser
        .parse_statement(&parse_quote!(is_power_of_two(val);))
        .unwrap();
    let model = parser.model;
    let [compute, constraint] = stmts[..] else {
        panic!("expected two statements, got {stmts:?}");
    };

    // The call computes one anonymous value: whether the constraint holds.
    let StmtKind::NativeCompute { func, args } = &model.statement(compute).kind else {
        panic!("expected a native computation");
    };
    assert!(func.is_ident("is_power_of_two"));
    assert_eq!(names(&model, &ids(args)), ["val"]);
    let [success] = model.statement(compute).results[..] else {
        panic!("expected one result");
    };
    assert_eq!(name(&model, success), "_");

    // The constraint requires that value to hold, and binds nothing.
    let StmtKind::Constraint { success: required } = model.statement(constraint).kind else {
        panic!("expected a constraint");
    };
    assert_eq!(required, success);
    assert!(model.statement(constraint).results.is_empty());
}

#[test]
fn parse_statement_rejects_other_statements() {
    let cases: [(syn::Stmt, &str); 8] = [
        (
            tail(parse_quote!({ f(x) })),
            "only constraint function calls are allowed as expressions in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(x + y;),
            "only constraint function calls are allowed as expressions in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(rewrite!(r, {});),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
        (
            tail(parse_quote!({ rewrite!(r, {}) })),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(erase!(r);),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(replace!(r, x);),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(other!(r);),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(
                fn helper() {}
            ),
            "items are not allowed in `#[rewrite_rule]` functions",
        ),
    ];
    for (stmt, message) in cases {
        assert_eq!(error(parser().parse_statement(&stmt)), message);
    }
}

// --- parse_local ---

#[test]
fn parse_local_binds_a_native_computation() {
    let mut parser = parser();
    let stmt = parser
        .parse_local(&local(parse_quote!(let (q, r) = divmod(x, y);)))
        .unwrap();
    let model = parser.model;
    let StmtKind::NativeCompute { func, args } = &model.statement(stmt).kind else {
        panic!("expected a native computation");
    };
    assert!(func.is_ident("divmod"));
    assert_eq!(names(&model, &ids(args)), ["x", "y"]);
    assert_eq!(names(&model, &model.statement(stmt).results), ["q", "r"]);
}

#[test]
fn parse_local_binds_an_op() {
    let mut parser = parser();
    let stmt = parser
        .parse_local(&local(parse_quote!(let r = op![Arith::mul(x, c)];)))
        .unwrap();
    let model = parser.model;
    assert_eq!(kind_name(&model, stmt), "op");
    assert_eq!(names(&model, &model.statement(stmt).results), ["r"]);
}

#[test]
fn parse_local_names_an_op_with_at() {
    for (stmt, results) in [
        (
            parse_quote!(let dm @ (q, rem) = op![Arith::divmod(x, y)];),
            &["q", "rem"][..],
        ),
        (parse_quote!(let s @ r = op![Arith::neg(x)];), &["r"][..]),
        (parse_quote!(let k @ () = op![Sink::sink(x)];), &[][..]),
    ] {
        let mut parser = parser();
        let op = parser.parse_local(&local(stmt)).unwrap();
        parser.push_body(op);
        let model = parser.model();
        assert_eq!(kind_name(model, op), "op");
        assert_eq!(names(model, &model.statement(op).results), results);
    }
}

#[test]
fn parse_local_resolves_the_initializer_before_binding() {
    assert_eq!(
        error(parser().parse_local(&local(parse_quote!(let x = op![Arith::neg(x)];)))),
        "`x` is used before the `let` that binds it"
    );
    // The same holds for the op's own name.
    assert_eq!(
        error(parser().parse_local(&local(parse_quote!(let k @ () = op![Sink::sink(k)];)))),
        "`k` is used before the `let` that binds it"
    );
}

#[test]
fn parse_local_rejects_malformed_bindings() {
    let cases = [
        (
            local(parse_quote!(let x;)),
            "all local bindings in `#[rewrite_rule]` functions must have an initializer",
        ),
        (
            local(parse_quote!(let x = f(a) else { return; };)),
            "diverging expressions are not allowed in `#[rewrite_rule]` functions",
        ),
        (
            local(parse_quote!(let x = 1;)),
            "only function and macro calls are allowed as initializers in `#[rewrite_rule]` functions",
        ),
    ];
    for (local, message) in cases {
        assert_eq!(error(parser().parse_local(&local)), message);
    }
}

// --- parse_pat ---

fn pat_names(pat: &syn::Pat) -> Vec<String> {
    RuleParser::parse_pat(pat)
        .unwrap()
        .into_iter()
        .map(|(name, ..)| name.map_or_else(|| "_".to_string(), |name| name.to_string()))
        .collect()
}

#[test]
fn parse_pat_flattens_tuples_and_keeps_wildcards() {
    assert_eq!(pat_names(&parse_quote!(x)), ["x"]);
    assert_eq!(pat_names(&parse_quote!((q, _, r))), ["q", "_", "r"]);
    assert_eq!(pat_names(&parse_quote!((a, (b, c)))), ["a", "b", "c"]);
}

#[test]
fn parse_pat_rejects_a_type_annotation() {
    let message = "type annotations are not supported in `#[rewrite_rule]` functions";
    for typed in [
        local(parse_quote!(let c: i64 = f();)),
        local(parse_quote!(let (q, r): (i64, i64) = f();)),
    ] {
        assert_eq!(error(RuleParser::parse_pat(&typed.pat)), message);
    }
}

#[test]
fn parse_pat_rejects_other_patterns() {
    assert_eq!(
        error(RuleParser::parse_pat(&parse_quote!(Some(x)))),
        "only identifier patterns are allowed in `#[rewrite_rule]` functions"
    );
}

/// Only the whole pattern of an `op![..]` can name something with `@`: a
/// native call's results, or a result inside a tuple, cannot.
#[test]
fn parse_pat_rejects_at_outside_an_op() {
    let message =
        "`@` can only name the statement of an `op![..]`, e.g. `let s @ (q, r) = op![..]`";
    assert_eq!(error(RuleParser::parse_pat(&parse_quote!(x @ _))), message);
    assert_eq!(
        error(RuleParser::parse_op_pat(&parse_quote!(s @ (q @ _, r)))),
        message
    );
    assert_eq!(
        error(parser().parse_local(&local(parse_quote!(let s @ q = f(x);)))),
        message
    );
}

#[test]
fn parse_op_pat_splits_the_op_name_from_its_results() {
    let (name, results) = RuleParser::parse_op_pat(&parse_quote!(dm @ (q, _))).unwrap();
    assert_eq!(name.unwrap(), "dm");
    assert_eq!(results.len(), 2);
    let (name, results) = RuleParser::parse_op_pat(&parse_quote!((q, r))).unwrap();
    assert!(name.is_none());
    assert_eq!(results.len(), 2);
}

// --- parse_ident ---

#[test]
fn parse_ident_accepts_a_plain_name() {
    let ident = pat_ident(parse_quote!(x));
    assert_eq!(RuleParser::parse_ident(&ident).unwrap(), "x");
}

#[test]
fn parse_ident_rejects_modifiers() {
    let cases = [
        (
            pat_ident(parse_quote!(mut x)),
            "mutable identifiers are not allowed in `#[rewrite_rule]` functions",
        ),
        (
            pat_ident(parse_quote!(ref x)),
            "by-ref identifiers are not allowed in `#[rewrite_rule]` functions",
        ),
    ];
    for (ident, message) in cases {
        assert_eq!(error(RuleParser::parse_ident(&ident)), message);
    }
}

// --- parse_native_call ---

#[test]
fn parse_native_call_returns_the_function_and_its_arguments() {
    let mut parser = parser();
    let (func, args) = parser
        .parse_native_call(&parse_quote!(my_mod::log2(val)))
        .unwrap();
    assert_eq!(func, parse_quote!(my_mod::log2));
    assert_eq!(names(&parser.model, &ids(&args)), ["val"]);
}

#[test]
fn parse_use_rejects_an_underscore_outside_an_op() {
    let message = "`_` can only be an argument of `op![..]` in `#[rewrite_rule]` functions";
    assert_eq!(
        error(parser().parse_native_call(&parse_quote!(is_power_of_two(_)))),
        message
    );
    assert_eq!(
        error(matched_parser().parse_rewrite_action(&parse_quote!(replace!(r, _)))),
        message
    );
}

#[test]
fn parse_native_call_rejects_a_callee_that_is_not_a_path() {
    let mut parser = parser();
    assert_eq!(
        error(parser.parse_native_call(&parse_quote!((f)(x)))),
        "function call must be a path"
    );
}

#[test]
fn parse_native_call_rejects_generic_arguments() {
    let message = "native functions in `#[rewrite_rule]` functions cannot have generic arguments";
    for call in [
        parse_quote!(convert::<i64>(x)),
        parse_quote!(math::<T>::log2(x)),
    ] {
        assert_eq!(error(parser().parse_native_call(&call)), message);
    }
}

// --- parse_use / parse_native_args ---

#[test]
fn parse_use_resolves_a_name() {
    let mut parser = parser();
    let x = parser.parse_use(&parse_quote!(x)).unwrap().value;
    let model = parser.model;
    assert!(model.is_hole(x));
    assert_eq!(name(&model, x), "x");
}

#[test]
fn parse_use_rejects_anything_but_a_name() {
    let mut parser = parser();
    assert_eq!(
        error(parser.parse_use(&parse_quote!(a::b))),
        "only single-segment paths are allowed as arguments in `#[rewrite_rule]` functions"
    );
    assert_eq!(
        error(parser.parse_use(&parse_quote!(1))),
        "only identifiers are allowed as arguments in `#[rewrite_rule]` functions"
    );
}

#[test]
fn parse_native_args_keeps_order_and_shares_repeated_names() {
    let mut parser = parser();
    let args: [syn::Expr; 3] = [parse_quote!(x), parse_quote!(y), parse_quote!(x)];
    let values = ids(&parser.parse_native_args(&args).unwrap());
    assert_eq!(values[0], values[2]);
    assert_eq!(names(&parser.model, &values), ["x", "y", "x"]);
}
