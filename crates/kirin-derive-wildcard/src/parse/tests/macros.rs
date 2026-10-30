use syn::parse_quote;

use crate::model::StmtKind;

use super::helpers::{error, ids, kind_name, matched_parser, name, names, parser, tail};

// --- parse_op ---

#[test]
fn parse_op_reads_the_op_name_and_arguments() {
    let mut parser = parser();
    let kind = parser
        .parse_op(&parse_quote!(op![Arith::add(x, y)]))
        .unwrap();
    let StmtKind::Op { op, args } = kind else {
        panic!("expected an op");
    };
    let expected: syn::Path = parse_quote!(Arith::add);
    assert_eq!(op, expected);
    assert_eq!(names(&parser.model, &ids(&args)), ["x", "y"]);
}

#[test]
fn parse_op_accepts_a_longer_dialect_path_and_a_raw_op() {
    for mac in [
        parse_quote!(op![kirin_arith::Arith::add(x, y)]),
        parse_quote!(op![Scf::r#if(c)]),
    ] {
        assert!(parser().parse_op(&mac).is_ok(), "{}", quote::quote!(#mac));
    }
}

#[test]
fn parse_op_makes_each_underscore_its_own_anonymous_hole() {
    let mut parser = parser();
    let kind = parser
        .parse_op(&parse_quote!(op![Arith::add(_, _)]))
        .unwrap();
    let StmtKind::Op { args, .. } = kind else {
        panic!("expected an op");
    };
    let model = parser.model;
    let [a, b] = ids(&args)[..] else {
        panic!("expected two arguments");
    };
    assert_ne!(a, b);
    for id in [a, b] {
        assert!(model.is_hole(id));
        assert!(model.value(id).name.is_none());
    }
}

#[test]
fn parse_op_rejects_malformed_invocations() {
    let cases: [(syn::Macro, &str); 7] = [
        (
            parse_quote!(other![add(x)]),
            "only `op!` macro invocations are allowed as initializers in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(op!(add(x))),
            "expected square brackets in `op![..]`",
        ),
        (
            parse_quote!(op![add(x)]),
            "ops in `op![..]` must name their dialect, e.g. `op![Arith::add(x, y)]`",
        ),
        (
            parse_quote!(op![<Arith as Ops>::add(x)]),
            "ops in `op![..]` must name their dialect, e.g. `op![Arith::add(x, y)]`",
        ),
        (
            parse_quote!(op![Arith::CondBr(x)]),
            "ops in `op![..]` are written in snake case: use `cond_br` instead of `CondBr`",
        ),
        (
            parse_quote!(op![Arith::add::<T>(x)]),
            "the op in `op![..]` cannot have generic arguments",
        ),
        (parse_quote!(op![(f)(x)]), "function call must be a path"),
    ];
    for (mac, message) in cases {
        assert_eq!(error(parser().parse_op(&mac)), message);
    }
}

#[test]
fn parse_op_requires_a_call() {
    assert!(parser().parse_op(&parse_quote!(op![add])).is_err());
}

// --- parse_rewrite / parse_rewrite_statement ---

#[test]
fn parse_rewrite_reads_the_root_and_body() {
    let mut parser = matched_parser();
    let rewrite = parser
        .parse_rewrite(&parse_quote!(rewrite!(r, {
            replace!(r, x);
            erase!(r);
        })))
        .unwrap();

    let model = parser.model;
    let StmtKind::Rewrite { root, body, .. } = &model.statement(rewrite).kind else {
        panic!("expected a rewrite");
    };
    // The root is the op defining `r`.
    assert_eq!(names(&model, &model.statement(*root).results), ["r"]);
    let kinds: Vec<_> = body.iter().map(|&id| kind_name(&model, id)).collect();
    assert_eq!(kinds, ["replace", "erase"]);
}

#[test]
fn parse_rewrite_rejects_malformed_invocations() {
    let cases: [(syn::Macro, &str); 2] = [
        (
            parse_quote!(rewrite!(r)),
            "expected `rewrite!(root, { .. })`",
        ),
        (
            parse_quote!(rewrite!(r, x)),
            "the body of `rewrite!` must be a block",
        ),
    ];
    for (mac, message) in cases {
        assert_eq!(error(matched_parser().parse_rewrite(&mac)), message);
    }
}

#[test]
fn parse_rewrite_statement_accepts_a_final_action_without_a_semicolon() {
    for stmt in [
        tail(parse_quote!({
            rewrite!(r, {
                replace!(r, x);
                erase!(r);
            })
        })),
        tail(parse_quote!({ replace!(r, x) })),
        tail(parse_quote!({ erase!(r) })),
    ] {
        assert!(matched_parser().parse_rewrite_statement(&stmt).is_ok());
    }
}

/// A lone `erase!` of an op without results: the op is also the root.
#[test]
fn parse_rewrite_statement_accepts_a_lone_erase_of_an_op_without_results() {
    let mut parser = parser();
    for id in parser
        .parse_statement(&parse_quote!(let k @ () = op![Sink::sink(x)];))
        .unwrap()
    {
        parser.push_body(id);
    }
    let sink = *parser.model().body().last().unwrap();
    let rewrite = parser
        .parse_rewrite_statement(&tail(parse_quote!({ erase!(k) })))
        .unwrap();

    let model = parser.model;
    let StmtKind::Rewrite { root, body, .. } = &model.statement(rewrite).kind else {
        panic!("expected a rewrite");
    };
    assert_eq!(*root, sink);
    let [erase] = body.as_slice() else {
        panic!("expected one action");
    };
    let StmtKind::Erase { stmt: op } = model.statement(*erase).kind else {
        panic!("expected an erase");
    };
    assert_eq!(op, sink);
}

#[test]
fn parse_rewrite_statement_rejects_other_final_statements() {
    let cases: [(syn::Stmt, &str); 4] = [
        (
            parse_quote!(rewrite!(r, {});),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
        (
            parse_quote!(erase!(r);),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
        (
            tail(parse_quote!({ other!(r) })),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
        (
            parse_quote!(f(x);),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
    ];
    for (stmt, message) in cases {
        assert_eq!(error(parser().parse_rewrite_statement(&stmt)), message);
    }
}

#[test]
fn parse_rewrite_statement_expands_a_lone_action_into_a_rewrite() {
    let mut parser = matched_parser();
    let rewrite = parser
        .parse_rewrite_statement(&tail(parse_quote!({ replace!(r, x) })))
        .unwrap();

    let model = parser.model;
    let StmtKind::Rewrite { root, body, .. } = &model.statement(rewrite).kind else {
        panic!("expected a rewrite");
    };
    let [action] = body.as_slice() else {
        panic!("expected one action");
    };
    let StmtKind::Replace { matched, .. } = model.statement(*action).kind else {
        panic!("expected a replace");
    };
    assert_eq!(*root, model.value(matched.value).stmt);
    assert_eq!(name(&model, matched.value), "r");
}

// --- parse_replacement ---

#[test]
fn parse_replacement_accepts_lets_and_actions() {
    let mut parser = matched_parser();
    let compute = parser
        .parse_replacement(&parse_quote!(let k = log2(val);))
        .unwrap();
    let op = parser
        .parse_replacement(&parse_quote!(let s = op![Bitwise::shl(x, k)];))
        .unwrap();
    let replace = parser
        .parse_replacement(&parse_quote!(replace!(r, s);))
        .unwrap();
    let model = parser.model;
    assert_eq!(kind_name(&model, compute), "compute");
    assert_eq!(kind_name(&model, op), "op");
    assert_eq!(kind_name(&model, replace), "replace");
}

#[test]
fn parse_replacement_rejects_other_statements() {
    let message = "only `let` bindings, `erase!(..);` and `replace!(..);` are allowed in the body of `rewrite!`";
    let cases: [syn::Stmt; 3] = [
        // No semicolon: the block's tail expression.
        tail(parse_quote!({ erase!(r) })),
        // A brace macro is a statement without needing a semicolon.
        parse_quote!(erase! { r }),
        parse_quote!(is_power_of_two(val);),
    ];
    for stmt in cases {
        assert_eq!(error(parser().parse_replacement(&stmt)), message);
    }
}

/// An op built in the body is never the root, replaced, or erased, so `@`
/// cannot name it, with or without results.
#[test]
fn parse_replacement_rejects_naming_a_built_op() {
    let message = "`@` cannot name an statement built in the body of `rewrite!`";
    let cases: [syn::Stmt; 2] = [
        parse_quote!(let n @ y = op![Arith::neg(r)];),
        parse_quote!(let k @ () = op![Sink::sink(r)];),
    ];
    for stmt in cases {
        assert_eq!(error(matched_parser().parse_replacement(&stmt)), message);
    }
}

/// Without `@`, an op without results is built by binding `()`.
#[test]
fn parse_replacement_builds_an_op_without_results_unnamed() {
    let mut parser = matched_parser();
    let op = parser
        .parse_replacement(&parse_quote!(let () = op![Sink::sink(r)];))
        .unwrap();
    let model = parser.model;
    assert_eq!(kind_name(&model, op), "op");
    assert!(model.statement(op).results.is_empty());
}

// --- parse_action ---

/// Both actions report the op they act on: the one defining the replaced
/// value, and the erased one.
#[test]
fn parse_action_reads_replace_and_erase() {
    let mut parser = matched_parser();
    let op = *parser.model().body().last().unwrap();
    let (replace, replaced) = parser
        .parse_rewrite_action(&parse_quote!(replace!(r, x)))
        .unwrap();
    let (erase, erased) = parser
        .parse_rewrite_action(&parse_quote!(erase!(r)))
        .unwrap();

    let model = parser.model;
    let StmtKind::Replace {
        matched,
        replacement,
    } = model.statement(replace).kind
    else {
        panic!("expected a replace");
    };
    assert_eq!(replaced, op);
    assert_eq!(names(&model, &ids(&[matched, replacement])), ["r", "x"]);
    let StmtKind::Erase { stmt: target } = model.statement(erase).kind else {
        panic!("expected an erase");
    };
    assert_eq!((target, erased), (op, op));
}

#[test]
fn parse_action_rejects_malformed_actions() {
    let cases: [(syn::Macro, &str); 5] = [
        (parse_quote!(erase!(a, b)), "expected `erase!(matched)`"),
        (
            parse_quote!(replace!(r)),
            "expected `replace!(matched, replacement)`",
        ),
        (
            parse_quote!(rewrite!(r, {})),
            "`rewrite!` macros cannot be nested",
        ),
        (
            parse_quote!(other!(r)),
            "expected `erase!` or `replace!` in the body of `rewrite!`",
        ),
        (
            parse_quote!(erase!(1)),
            "`erase!` can only act on an `op![..]` before the rewrite, or one of its results",
        ),
    ];
    for (mac, message) in cases {
        assert_eq!(error(parser().parse_rewrite_action(&mac)), message);
    }
}
