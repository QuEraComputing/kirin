//! Proof of concept: removing double negations with `#[rewrite_rule]` rules,
//! driven to a fixpoint inside `Pipeline::run_pass` in every walk order.

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::SSAKind;
use kirin_ir::walk::{BFS, KeepOrderReverseBFS, PostOrder, PreOrder, ReverseBFS, WalkOrder};
use kirin_rewrite::{
    DriverError, NativeRegistry, NativeValue, RewriteDriver, RewriteSummary, RuleErrorKind, RuleSet,
};
use kirin_test_languages::ArithFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

/// `-(-x)` is `x`: integer negation wraps and float negation flips the sign
/// bit, so negating twice is exact. The two `neg`s are left dead for the
/// driver's dead-code cleanup.
#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
    })
}

/// Only named by `double_neg_of_argument`; the driver calls the
/// `is_block_argument` registered in [`natives`].
fn is_block_argument(_x: i64) -> bool {
    unreachable!("the rule only names it")
}

/// `double_neg`, restricted by a native check to negations of a block
/// argument.
#[rewrite_rule(language = L)]
fn double_neg_of_argument() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    is_block_argument(x);
    rewrite!(r, {
        replace!(r, x);
    })
}

/// `double_neg` erasing its root, which the driver is visiting.
#[rewrite_rule(language = L)]
fn double_neg_erasing_root() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
        erase!(r);
    })
}

/// `double_neg` erasing the inner `neg`, a matched producer, which fails if
/// anything else still uses it.
#[rewrite_rule(language = L)]
fn double_neg_erasing_producer() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
        erase!(a);
    })
}

/// Matches `x + x` only to exercise a hole used twice; replacing it with `x`
/// is not a valid rewrite.
#[rewrite_rule(language = L)]
fn add_self() {
    let r = op![Arith::add(x, x)];
    rewrite!(r, {
        replace!(r, x);
    })
}

fn natives() -> NativeRegistry<L> {
    NativeRegistry::new().check("is_block_argument", |stage, args| {
        let [NativeValue::Value(value)] = args else {
            return None;
        };
        let argument = matches!(value.get_info(stage)?.kind(), SSAKind::BlockArgument(..));
        Some(vec![NativeValue::Bool(argument)])
    })
}

fn neg_rules() -> RuleSet<L> {
    RuleSet::build(NativeRegistry::new(), |stage| vec![double_neg(stage)])
        .expect("the rules are valid")
}

/// A one-function program on `%x: i64` with `body` before `ret`.
fn program(body: &str, ret: &str) -> String {
    format!(
        "stage @arith fn @f(i64) -> i64;\n\
         specialize @arith fn @f(i64) -> i64 cfg {{\n\
           ^entry(%x: i64) {{\n{body}\n    ret {ret};\n  }}\n}}\n"
    )
}

fn parse(input: &str) -> (Pipeline<StageInfo<L>>, CompileStage) {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(input).expect("the program parses");
    let stage = pipeline.stage_by_name("arith").expect("the stage exists");
    (pipeline, stage)
}

/// Run `rules` over `input` in order `O`, returning the printed result.
fn rewrite<O: WalkOrder>(rules: &RuleSet<L>, input: &str) -> (String, RewriteSummary) {
    let (mut pipeline, stage) = parse(input);
    let driver = RewriteDriver::new(rules);
    let summary = pipeline
        .run_pass(stage, |rewriter| driver.run::<O>(rewriter))
        .expect("the pass succeeds");
    (pipeline.sprint(), summary)
}

/// `input` printed, to compare against without depending on spacing.
fn printed(input: &str) -> String {
    parse(input).0.sprint()
}

/// Check `rules` turn `input` into `expected` in every walk order.
fn assert_rewrites(rules: &RuleSet<L>, input: &str, expected: &str) {
    let expected = printed(expected);
    assert_eq!(rewrite::<PreOrder>(rules, input).0, expected, "PreOrder");
    assert_eq!(rewrite::<PostOrder>(rules, input).0, expected, "PostOrder");
    assert_eq!(rewrite::<BFS>(rules, input).0, expected, "BFS");
    assert_eq!(
        rewrite::<ReverseBFS>(rules, input).0,
        expected,
        "ReverseBFS"
    );
    assert_eq!(
        rewrite::<KeepOrderReverseBFS>(rules, input).0,
        expected,
        "KeepOrderReverseBFS"
    );
}

#[test]
fn double_negation_is_removed() {
    let input = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    assert_rewrites(&neg_rules(), &input, &program("", "%x"));
}

#[test]
fn a_chain_of_negations_reduces_by_parity() {
    let rules = neg_rules();
    let three = program(
        "    %a = neg %x -> i64;\n    %b = neg %a -> i64;\n    %c = neg %b -> i64;",
        "%c",
    );
    // Left to right, `%b` folds first and `%c = neg %x` survives; right to
    // left, `%c` folds into `%a` and `%a = neg %x` survives. Both are `-x`.
    let c_survives = printed(&program("    %c = neg %x -> i64;", "%c"));
    let a_survives = printed(&program("    %a = neg %x -> i64;", "%a"));
    assert_eq!(
        rewrite::<PreOrder>(&rules, &three).0,
        c_survives,
        "PreOrder"
    );
    assert_eq!(
        rewrite::<PostOrder>(&rules, &three).0,
        c_survives,
        "PostOrder"
    );
    assert_eq!(rewrite::<BFS>(&rules, &three).0, c_survives, "BFS");
    assert_eq!(
        rewrite::<KeepOrderReverseBFS>(&rules, &three).0,
        c_survives,
        "KeepOrderReverseBFS"
    );
    assert_eq!(
        rewrite::<ReverseBFS>(&rules, &three).0,
        a_survives,
        "ReverseBFS"
    );

    let four = program(
        "    %a = neg %x -> i64;\n    %b = neg %a -> i64;\n    %c = neg %b -> i64;\n    %d = neg %c -> i64;",
        "%d",
    );
    assert_rewrites(&rules, &four, &program("", "%x"));
}

#[test]
fn the_fixpoint_ends_with_a_sweep_that_changes_nothing() {
    // Sweep 1: `double_neg` folds `%b` into `%x`; the dead-code cleanup then
    // erases `%b` and, with it unused, `%a`. Sweep 2 changes nothing.
    let input = program(
        "    %a = neg %x -> i64;\n    %b = neg %a -> i64;\n    %c = neg %b -> i64;",
        "%c",
    );
    let (_, summary) = rewrite::<PostOrder>(&neg_rules(), &input);
    assert_eq!(
        summary,
        RewriteSummary {
            sweeps: 2,
            applied: 1,
            erased: 2,
        }
    );
}

#[test]
fn a_program_without_matches_is_left_alone() {
    let input = program("    %a = neg %x -> i64;\n    %b = add %a, %x -> i64;", "%b");
    let (output, summary) = rewrite::<PostOrder>(&neg_rules(), &input);
    assert_eq!(output, printed(&input));
    assert_eq!(
        summary,
        RewriteSummary {
            sweeps: 1,
            applied: 0,
            erased: 0,
        }
    );
}

#[test]
fn every_function_in_the_stage_is_rewritten() {
    let input = "stage @arith fn @f(i64) -> i64;\n\
                 stage @arith fn @g(i64) -> i64;\n\
                 specialize @arith fn @f(i64) -> i64 cfg {\n\
                   ^entry(%x: i64) {\n    %a = neg %x -> i64;\n    %b = neg %a -> i64;\n    ret %b;\n  }\n}\n\
                 specialize @arith fn @g(i64) -> i64 cfg {\n\
                   ^entry(%y: i64) {\n    %c = neg %y -> i64;\n    %d = neg %c -> i64;\n    ret %d;\n  }\n}\n";
    let expected = "stage @arith fn @f(i64) -> i64;\n\
                    stage @arith fn @g(i64) -> i64;\n\
                    specialize @arith fn @f(i64) -> i64 cfg {\n  ^entry(%x: i64) {\n    ret %x;\n  }\n}\n\
                    specialize @arith fn @g(i64) -> i64 cfg {\n  ^entry(%y: i64) {\n    ret %y;\n  }\n}\n";
    let (output, summary) = rewrite::<PostOrder>(&neg_rules(), input);
    assert_eq!(output, printed(expected));
    assert_eq!(
        (summary.applied, summary.erased),
        (2, 4),
        "per function: one double_neg, two dead negs"
    );
}

#[test]
fn a_hole_used_twice_matches_only_the_same_value() {
    let rules = RuleSet::build(NativeRegistry::new(), |stage| vec![add_self(stage)]).unwrap();
    let same = program("    %a = add %x, %x -> i64;", "%a");
    assert_rewrites(&rules, &same, &program("", "%x"));

    let different = program("    %n = neg %x -> i64;\n    %a = add %x, %n -> i64;", "%a");
    assert_rewrites(&rules, &different, &different);
}

#[test]
fn a_native_check_decides_whether_the_rule_applies() {
    let rules = RuleSet::build(natives(), |stage| vec![double_neg_of_argument(stage)]).unwrap();
    let of_argument = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    assert_rewrites(&rules, &of_argument, &program("", "%x"));

    let of_sum = program(
        "    %s = add %x, %x -> i64;\n    %a = neg %s -> i64;\n    %b = neg %a -> i64;",
        "%b",
    );
    assert_rewrites(&rules, &of_sum, &of_sum);
}

#[test]
fn an_unregistered_native_function_rejects_the_rule() {
    let error = RuleSet::build(NativeRegistry::new(), |stage| {
        vec![double_neg_of_argument(stage)]
    })
    .err()
    .expect("`is_block_argument` is not registered");
    assert_eq!(error.rule.as_deref(), Some("double_neg_of_argument"));
    assert_eq!(
        error.kind,
        RuleErrorKind::UnknownCheck("is_block_argument".to_string())
    );
}

#[test]
fn a_rule_erasing_its_root_is_rejected() {
    let error = RuleSet::build(NativeRegistry::new(), |stage| {
        vec![double_neg_erasing_root(stage)]
    })
    .err()
    .expect("the rule erases its root");
    assert_eq!(error.kind, RuleErrorKind::ErasesRoot);
}

#[test]
fn a_failed_rewrite_quarantines_the_stage() {
    // `%a` is also used by the `add`, so erasing it fails.
    let input = program(
        "    %a = neg %x -> i64;\n    %b = neg %a -> i64;\n    %s = add %a, %b -> i64;",
        "%s",
    );
    let (mut pipeline, stage) = parse(&input);
    let rules = RuleSet::build(NativeRegistry::new(), |stage| {
        vec![double_neg_erasing_producer(stage)]
    })
    .unwrap();
    let driver = RewriteDriver::new(&rules);

    let mut failure = None;
    let result = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<PostOrder>(rewriter);
        failure = result.clone().err();
        result
    });

    assert!(result.is_err());
    assert!(matches!(
        failure,
        Some(DriverError::Rewrite {
            error: RewriteError::StatementResultsInUse(_),
            ..
        })
    ));
    assert_eq!(pipeline.stage_status(stage), Some(StageStatus::Poisoned));
}

#[test]
fn running_out_of_sweeps_quarantines_the_stage() {
    let input = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    let (mut pipeline, stage) = parse(&input);
    let rules = neg_rules();
    let driver = RewriteDriver::new(&rules).max_sweeps(1);

    let mut failure = None;
    let result = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<PostOrder>(rewriter);
        failure = result.clone().err();
        result
    });

    assert!(result.is_err());
    assert!(matches!(
        failure,
        Some(DriverError::BudgetExhausted { sweeps: 1, .. })
    ));
    assert_eq!(pipeline.stage_status(stage), Some(StageStatus::Poisoned));
}
