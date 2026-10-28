use quote::quote;
use syn::parse_quote;

use super::RuleCodegen;
use crate::model::{RuleModel, StmtId, StmtKind, ValueId, WalkResult};
use crate::parse::RuleParser;
use crate::rewrite_rule::RuleArgs;

fn args() -> RuleArgs {
    RuleArgs {
        language: parse_quote!(L),
        crate_path: parse_quote!(::kirin_wildcard),
    }
}

/// The rule's first statement that is not a hole: the first one it writes.
fn first_written(model: &RuleModel) -> StmtId {
    *model
        .body()
        .iter()
        .find(|&&s| !matches!(model.statement(s).kind, StmtKind::Hole))
        .expect("the rule writes a statement")
}

/// The rule's first native call.
fn first_native_call(model: &RuleModel) -> StmtId {
    *model
        .body()
        .iter()
        .find(|&&s| matches!(model.statement(s).kind, StmtKind::NativeCompute { .. }))
        .expect("the rule makes a native call")
}

/// The arguments and results of the rule's first native call.
fn first_call(model: &RuleModel) -> (Vec<ValueId>, Vec<ValueId>) {
    let stmt = model.statement(first_native_call(model));
    let StmtKind::NativeCompute { args, .. } = &stmt.kind else {
        panic!("expected a native computation");
    };
    (
        args.iter().map(|arg| arg.value).collect(),
        stmt.results.clone(),
    )
}

// --- signature ---

/// The rule keeps its attributes, visibility and name, takes the stage to
/// build its pattern in, and returns the pattern statement.
#[test]
fn signature_takes_the_stage_and_returns_the_pattern() {
    let item: syn::ItemFn = parse_quote!(
        #[inline]
        pub fn add_zero() {
            let r = op![Arith::add(x, y)];
            replace!(r, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let expected = quote! {
        #[inline]
        pub fn add_zero(
            stage: &mut impl ::kirin_wildcard::__private::ir::AsBuildStage<::kirin_wildcard::PatternLanguage<L>>,
        ) -> ::kirin_wildcard::__private::ir::Statement
    };
    assert_eq!(codegen.signature().to_string(), expected.to_string());
}

// --- value_ident ---

#[test]
fn value_ident_prefixes_the_value_name() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::add(x, y)];
            let (q, _) = divmod(x, y);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let (call_args, results) = first_call(model);
    assert_eq!(codegen.value_ident(call_args[0]), "value_x");
    assert_eq!(codegen.value_ident(call_args[1]), "value_y");
    assert_eq!(codegen.value_ident(results[0]), "value_q");
}

#[test]
fn value_ident_numbers_an_anonymous_value() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::add(x, y)];
            let (q, _) = divmod(x, y);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let (_, results) = first_call(model);
    let anonymous = results[1];
    assert_eq!(
        codegen.value_ident(anonymous),
        format!("value_{}", anonymous.index())
    );
}

#[test]
fn value_ident_is_the_same_for_every_use() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::add(x, y)];
            let (q, _) = divmod(x, y);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let (_, results) = first_call(model);
    assert_eq!(
        codegen.value_ident(results[1]),
        codegen.value_ident(results[1])
    );
}

#[test]
fn value_ident_strips_a_raw_prefix() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::neg(x)];
            let r#type = f(x);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let (_, results) = first_call(model);
    assert_eq!(codegen.value_ident(results[0]), "value_type");
}

// --- hole ---

#[test]
fn hole_builds_an_operand_in_the_stage() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::neg(x)];
            let q = f(x);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let (call_args, _) = first_call(model);
    let hole = model.value(call_args[0]).stmt;
    let statement = codegen.statement_ident(hole);
    let expected = quote! {
        let #statement = stage.statement_arena().next_id();
        let value_x: ::kirin_wildcard::__private::ir::ResultValue = stage
            .ssa()
            .name("x")
            .ty(<__Type as ::kirin_wildcard::__private::ir::Placeholder>::placeholder())
            .kind(::kirin_wildcard::__private::ir::BuilderSSAKind::Result(#statement, 0usize))
            .new()
            .into();
        stage
            .statement()
            .definition(::kirin_wildcard::Hole::<__Type>::new_hole(value_x))
            .new();
    };
    assert_eq!(codegen.hole(hole).to_string(), expected.to_string());
}

#[test]
fn results_leaves_an_anonymous_value_unnamed() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::add(x, y)];
            let (q, _) = divmod(x, y);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let (code, idents) = codegen.results(first_native_call(model));
    let (_, results) = first_call(model);
    assert_eq!(
        idents,
        [
            codegen.value_ident(results[0]),
            codegen.value_ident(results[1])
        ]
    );
    // Only `q` is named.
    assert_eq!(code.to_string().matches(". name").count(), 1, "{code}");
}

// --- op ---

/// The code `op` generates for the rule's first statement, an op.
fn first_op(item: &syn::ItemFn) -> (String, String) {
    let codegen = RuleCodegen::new(RuleParser::parse(item).unwrap(), item.clone(), args());
    let model = &codegen.model;
    let stmt = first_written(model);
    let StmtKind::Op { op, args: operands } = &model.statement(stmt).kind else {
        panic!("expected an op");
    };
    let tokens = codegen.op(stmt, op, operands);
    (tokens.to_string(), codegen.results(stmt).0.to_string())
}

#[test]
fn op_builds_the_statement_with_the_dialect_constructor() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let r = op![Arith::add(x, y)];
            replace!(r, x)
        }
    );
    let (tokens, results) = first_op(&item);
    let definition = quote! {
        stage
            .statement()
            .definition(::kirin_wildcard::PatternLanguage::<L>::Op(::core::convert::From::from(
                Arith::new_add(
                    <::kirin_wildcard::__private::ir::SSAValue>::from(value_x),
                    <::kirin_wildcard::__private::ir::SSAValue>::from(value_y),
                    value_r
                )
            )))
            .new();
    };
    assert_eq!(tokens, format!("{results} {definition}"));
}

#[test]
fn op_maps_a_raw_op_and_keeps_the_dialect_path() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let r = op![scf::Scf::r#if(c)];
            replace!(r, c)
        }
    );
    let (tokens, _) = first_op(&item);
    let constructor = quote!(scf::Scf::new_if).to_string();
    assert!(tokens.contains(&constructor), "{tokens}");
}

// --- native_name / native_compute ---

#[test]
fn native_name_spells_out_the_whole_path() {
    let cases: [(syn::Path, &str); 4] = [
        (parse_quote!(log2), "log2"),
        (parse_quote!(math::log2), "math::log2"),
        (parse_quote!(::std::f64::sqrt), "::std::f64::sqrt"),
        (parse_quote!(r#type::convert), "type::convert"),
    ];
    for (path, name) in cases {
        assert_eq!(RuleCodegen::native_name(&path), name);
    }
}

#[test]
fn native_compute_builds_a_named_native_call() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let s = op![Arith::add(x, y)];
            let (q, r) = math::divmod(x, y);
            replace!(s, x)
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;
    let stmt = first_native_call(model);
    let StmtKind::NativeCompute {
        func,
        args: operands,
    } = &model.statement(stmt).kind
    else {
        panic!("expected a native computation");
    };

    let (results, _) = codegen.results(stmt);
    let expected = quote! {
        let _ = math::divmod;
        #results
        stage
            .statement()
            .definition(::kirin_wildcard::NativeCall::<__Type>::new_native_call(
                ::std::string::String::from("math::divmod"),
                ::std::vec![
                    <::kirin_wildcard::__private::ir::SSAValue>::from(value_x),
                    <::kirin_wildcard::__private::ir::SSAValue>::from(value_y)
                ],
                ::std::vec![value_q, value_r],
            ))
            .new();
    };
    assert_eq!(
        codegen.native_compute(stmt, func, operands).to_string(),
        expected.to_string()
    );
}

// --- constraint / replace / erase / rewrite ---

#[test]
fn constraint_replace_and_erase_use_their_builders() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let r = op![Arith::add(x, y)];
            is_power_of_two(x);
            rewrite!(r, {
                replace!(r, x);
                erase!(r);
            })
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let constraint = *model
        .body()
        .iter()
        .find(|&&s| matches!(model.statement(s).kind, StmtKind::Constraint { .. }))
        .unwrap();
    let StmtKind::Constraint { success } = model.statement(constraint).kind else {
        panic!("expected a constraint");
    };
    let (constraint_id, success_id) = (
        codegen.statement_ident(constraint),
        codegen.value_ident(success),
    );
    let expected = quote! {
        let #constraint_id = ::kirin_wildcard::Constraint::<__Type>::new(stage, #success_id).id;
    };
    assert_eq!(
        codegen.constraint(constraint, success).to_string(),
        expected.to_string()
    );

    let rewrite = *model.body().last().unwrap();
    let StmtKind::Rewrite { body, .. } = &model.statement(rewrite).kind else {
        panic!("expected a rewrite");
    };
    let [replace, erase] = body[..] else {
        panic!("expected a replace and an erase");
    };
    let StmtKind::Replace {
        matched,
        replacement,
    } = model.statement(replace).kind
    else {
        panic!("expected a replace");
    };
    let replace_id = codegen.statement_ident(replace);
    let expected = quote! {
        let #replace_id = ::kirin_wildcard::Replace::<__Type>::new(stage, value_r, value_x).id;
    };
    assert_eq!(
        codegen
            .replace(replace, matched.value, replacement.value)
            .to_string(),
        expected.to_string()
    );

    // The erased op is named by its position in the pattern body: after the
    // holes `x` and `y`.
    let StmtKind::Erase { stmt: op } = model.statement(erase).kind else {
        panic!("expected an erase");
    };
    assert_eq!(model.position(op), 2);
    let erase_id = codegen.statement_ident(erase);
    let expected = quote! {
        let #erase_id = ::kirin_wildcard::Erase::<__Type>::new(stage, 2).id;
    };
    assert_eq!(codegen.erase(erase, op).to_string(), expected.to_string());
}

#[test]
fn rewrite_builds_its_body_block_then_the_rewrite() {
    let item: syn::ItemFn = parse_quote!(
        fn add_zero() {
            let r = op![Arith::add(x, y)];
            rewrite!(r, {
                replace!(r, x);
                erase!(r);
            })
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let rewrite = *model.body().last().unwrap();
    let StmtKind::Rewrite { root, body, .. } = &model.statement(rewrite).kind else {
        panic!("expected a rewrite");
    };
    let rewrite_id = codegen.statement_ident(rewrite);
    let actions = body.iter().map(|&s| codegen.statement(s));
    let action_ids = body.iter().map(|&s| codegen.statement_ident(s));
    // The root is named by its position in the pattern body, after the
    // holes `x` and `y`.
    let expected = quote! {
        #(#actions)*
        let rewrite_body = stage.block().name("add_zero_rw")#(.stmt(#action_ids))*.new();
        let #rewrite_id = ::kirin_wildcard::Rewrite::<__Type>::new(stage, 2, rewrite_body).id;
    };
    assert_eq!(
        codegen.rewrite(rewrite, *root, body, None).to_string(),
        expected.to_string()
    );
}

#[test]
fn rewrite_ends_its_body_block_in_the_walk_result() {
    let item: syn::ItemFn = parse_quote!(
        fn neg_skip() {
            let r = op![Arith::neg(x)];
            rewrite!(r, {
                replace!(r, x);
                skip!()
            })
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let rewrite = *model.body().last().unwrap();
    let StmtKind::Rewrite {
        root,
        body,
        walk_result,
    } = &model.statement(rewrite).kind
    else {
        panic!("expected a rewrite");
    };
    let walk_result = walk_result.expect("the body ends in `skip!()`");
    let rewrite_id = codegen.statement_ident(rewrite);
    let walk_result_id = codegen.statement_ident(walk_result);
    let [replace] = body.as_slice() else {
        panic!("expected one action");
    };
    let replace_code = codegen.statement(*replace);
    let replace_id = codegen.statement_ident(*replace);
    let expected = quote! {
        #replace_code
        let #walk_result_id = ::kirin_wildcard::Skip::<__Type>::new(stage).id;
        let rewrite_body = stage.block().name("neg_skip_rw").stmt(#replace_id).terminator(#walk_result_id).new();
        let #rewrite_id = ::kirin_wildcard::Rewrite::<__Type>::new(stage, 1, rewrite_body).id;
    };
    assert_eq!(
        codegen
            .rewrite(rewrite, *root, body, Some(walk_result))
            .to_string(),
        expected.to_string()
    );
}

#[test]
fn walk_result_builds_the_walk_result_terminator() {
    let item: syn::ItemFn = parse_quote!(
        fn neg_decide() {
            let r = op![Arith::neg(x)];
            rewrite!(r, {
                replace!(r, x);
                decide(x)
            })
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let walk_result = model.walk_result().expect("the body ends in a native call");
    let StmtKind::WalkResult(WalkResult::Value(value)) = &model.statement(walk_result).kind else {
        panic!("expected a walk result value");
    };
    let walk_result_id = codegen.statement_ident(walk_result);
    let value_id = codegen.value_ident(value.value);
    let expected = quote! {
        let #walk_result_id = ::kirin_wildcard::WalkResultOf::<__Type>::new(stage, #value_id).id;
    };
    assert_eq!(
        codegen.statement(walk_result).to_string(),
        expected.to_string()
    );
}

// --- pattern ---

#[test]
fn pattern_builds_its_body_with_the_rewrite_as_terminator() {
    let item: syn::ItemFn = parse_quote!(
        fn add_zero() {
            let r = op![Arith::add(x, y)];
            rewrite!(r, {
                replace!(r, x);
                erase!(r);
            })
        }
    );
    let codegen = RuleCodegen::new(RuleParser::parse(&item).unwrap(), item, args());
    let model = &codegen.model;

    let body = model.body();
    let code = body.iter().map(|&s| codegen.statement(s));
    let (rewrite, matched) = body.split_last().unwrap();
    let ids = matched.iter().map(|&s| codegen.statement_ident(s));
    let rewrite_id = codegen.statement_ident(*rewrite);
    let expected = quote! {{
        #(#code)*
        let pattern_body = stage
            .block()
            .name("add_zero_pat")
            #(.stmt(#ids))*
            .terminator(#rewrite_id)
            .new();
        let pattern_name = stage
            .symbol_table_mut()
            .intern(::std::string::String::from("add_zero"));
        ::kirin_wildcard::Pattern::<__Type>::new(stage, pattern_name, pattern_body).id
    }};
    assert_eq!(codegen.pattern().to_string(), expected.to_string());
}

// --- whole expansion ---

/// The whole expansion of a rule using every kind of statement: holes, a
/// named op, `_`, a constraint, a native call and an op built in the
/// rewrite's body, `replace!` and `erase!`. A snapshot, so a change to the
/// generated code shows up as a readable diff.
#[test]
fn generate_expands_a_rule_using_every_statement() {
    let item: syn::ItemFn = parse_quote!(
        /// Rewrites `-x + _` into `-log2(x)` when `x` is a power of two.
        pub fn rule() {
            let a = op![Arith::neg(x)];
            let s @ r = op![Arith::add(a, _)];
            is_power_of_two(x);
            rewrite!(s, {
                let k = math::log2(x);
                let n = op![Arith::neg(k)];
                replace!(r, n);
                erase!(s);
            })
        }
    );
    let code = RuleCodegen::generate(RuleParser::parse(&item).unwrap(), item, args());
    insta::assert_snapshot!(kirin_test_utils::rustfmt(code.to_string()));
}
