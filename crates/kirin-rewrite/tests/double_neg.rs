//! Proof of concept: removing double negations with `#[rewrite_rule]` rules,
//! driven to a fixpoint inside `Pipeline::run_pass` in every walk order.

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::SSAKind;
use kirin_ir::walk::{BFS, KeepOrderReverseBFS, PostOrder, PreOrder, ReverseBFS, WalkOrder};
use kirin_rewrite::{
    DriverError, NativeValue, PatternRule, RewriteDriver, RewriteSummary, RuleErrorKind, RuleSet,
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

/// Whether `x` is a block argument.
fn is_block_argument(stage: &StageInfo<L>, x: SSAValue) -> bool {
    matches!(x.expect_info(stage).kind(), SSAKind::BlockArgument(..))
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

/// The position of `x` among its block's arguments, or `None` if `x` is not
/// a block argument.
fn argument_index(stage: &StageInfo<L>, x: SSAValue) -> Option<i64> {
    match x.expect_info(stage).kind() {
        SSAKind::BlockArgument(_, index) => Some(*index as i64),
        _ => None,
    }
}

fn is_zero(_stage: &StageInfo<L>, index: i64) -> bool {
    index == 0
}

/// `double_neg` of the first block argument: one check computes an `i64`,
/// failing the match if there is none, and another reads it.
#[rewrite_rule(language = L)]
fn double_neg_of_first_argument() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    let index = argument_index(x)?;
    is_zero(index);
    rewrite!(r, {
        replace!(r, x);
    })
}

/// The number of operands of the statement defining `x`, or 0 for a block
/// argument. Never fails; it returns a `Result` to exercise a `?` check
/// returning one.
fn operand_count(stage: &StageInfo<L>, x: SSAValue) -> Result<i64, std::convert::Infallible> {
    Ok(match x.expect_info(stage).kind() {
        SSAKind::Result(stmt, _) => stmt.expect_info(stage).definition().arguments().count() as i64,
        _ => 0,
    })
}

/// `double_neg` of a value defined without operands: a `?` check returning
/// a `Result` binds its `Ok` value for another check.
#[rewrite_rule(language = L)]
fn double_neg_of_leaf() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    let count = operand_count(x)?;
    is_zero(count);
    rewrite!(r, {
        replace!(r, x);
    })
}

/// What `argument_position` fails with.
#[derive(Debug, PartialEq)]
struct NotAnArgument;

impl std::fmt::Display for NotAnArgument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("not a block argument")
    }
}

impl std::error::Error for NotAnArgument {}

/// `argument_index`, but failing with an error for anything other than a
/// block argument.
fn argument_position(stage: &StageInfo<L>, x: SSAValue) -> Result<i64, NotAnArgument> {
    argument_index(stage, x).ok_or(NotAnArgument)
}

/// `double_neg` of the first block argument, whose check fails the run
/// rather than the match for anything else.
#[rewrite_rule(language = L)]
fn double_neg_requiring_argument() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    let index = argument_position(x)?;
    is_zero(index);
    rewrite!(r, {
        replace!(r, x);
    })
}

/// `double_neg` passing the SSA value `x` to `is_zero`, which takes an
/// `i64`.
#[rewrite_rule(language = L)]
fn double_neg_of_zero() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    is_zero(x);
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

/// Matches `x + y` only to exercise two holes binding the same value;
/// replacing it with `x` is not a valid rewrite.
#[rewrite_rule(language = L)]
fn add_any() {
    let r = op![Arith::add(x, y)];
    rewrite!(r, {
        replace!(r, x);
    })
}

/// Matches `-x + -y` only to exercise two ops that could match the same
/// statement; replacing it with `x` is not a valid rewrite.
#[rewrite_rule(language = L)]
fn add_of_negs() {
    let a = op![Arith::neg(x)];
    let b = op![Arith::neg(y)];
    let r = op![Arith::add(a, b)];
    rewrite!(r, {
        replace!(r, x);
    })
}

fn neg_rules() -> RuleSet<L> {
    RuleSet::build(|stage| vec![double_neg(stage)]).expect("the rules are valid")
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
    let rules = RuleSet::build(|stage| vec![add_self(stage)]).unwrap();
    let same = program("    %a = add %x, %x -> i64;", "%a");
    assert_rewrites(&rules, &same, &program("", "%x"));

    let different = program("    %n = neg %x -> i64;\n    %a = add %x, %n -> i64;", "%a");
    assert_rewrites(&rules, &different, &different);
}

#[test]
fn two_holes_may_bind_the_same_value() {
    let rules = RuleSet::build(|stage| vec![add_any(stage)]).unwrap();
    let input = program("    %a = add %x, %x -> i64;", "%a");
    assert_rewrites(&rules, &input, &program("", "%x"));
}

#[test]
fn two_ops_never_match_the_same_statement() {
    let rules = RuleSet::build(|stage| vec![add_of_negs(stage)]).unwrap();
    let same = program("    %n = neg %x -> i64;\n    %a = add %n, %n -> i64;", "%a");
    assert_rewrites(&rules, &same, &same);

    let different = program(
        "    %n = neg %x -> i64;\n    %m = neg %x -> i64;\n    %a = add %n, %m -> i64;",
        "%a",
    );
    assert_rewrites(&rules, &different, &program("", "%x"));
}

#[test]
fn a_native_check_decides_whether_the_rule_applies() {
    let rules = RuleSet::build(|stage| vec![double_neg_of_argument(stage)]).unwrap();
    let of_argument = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    assert_rewrites(&rules, &of_argument, &program("", "%x"));

    let of_sum = program(
        "    %s = add %x, %x -> i64;\n    %a = neg %s -> i64;\n    %b = neg %a -> i64;",
        "%b",
    );
    assert_rewrites(&rules, &of_sum, &of_sum);
}

#[test]
fn a_native_check_passes_an_i64_to_another() {
    let rules = RuleSet::build(|stage| vec![double_neg_of_first_argument(stage)]).unwrap();
    let of_argument = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    assert_rewrites(&rules, &of_argument, &program("", "%x"));

    let of_sum = program(
        "    %s = add %x, %x -> i64;\n    %a = neg %s -> i64;\n    %b = neg %a -> i64;",
        "%b",
    );
    assert_rewrites(&rules, &of_sum, &of_sum);
}

#[test]
fn a_native_check_returning_ok_binds_its_value() {
    let rules = RuleSet::build(|stage| vec![double_neg_of_leaf(stage)]).unwrap();
    let of_argument = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    assert_rewrites(&rules, &of_argument, &program("", "%x"));

    let of_sum = program(
        "    %s = add %x, %x -> i64;\n    %a = neg %s -> i64;\n    %b = neg %a -> i64;",
        "%b",
    );
    assert_rewrites(&rules, &of_sum, &of_sum);
}

#[test]
fn a_native_check_returning_err_fails_the_run() {
    let rules = RuleSet::build(|stage| vec![double_neg_requiring_argument(stage)]).unwrap();
    let input = program(
        "    %s = add %x, %x -> i64;\n    %a = neg %s -> i64;\n    %b = neg %a -> i64;",
        "%b",
    );
    let (mut pipeline, stage) = parse(&input);
    let driver = RewriteDriver::new(&rules);

    let mut failure = None;
    let result = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<PostOrder>(rewriter);
        failure = result.clone().err();
        result
    });

    assert!(result.is_err());
    let failure = failure.expect("the driver fails");
    let source = std::error::Error::source(&failure).and_then(|source| source.downcast_ref());
    assert_eq!(source, Some(&NotAnArgument));
    let DriverError::NativeFailed { name, error, .. } = failure else {
        panic!("expected `NativeFailed`, got {failure:?}");
    };
    assert_eq!(name, "argument_position");
    assert_eq!(error.error().downcast_ref(), Some(&NotAnArgument));
}

#[test]
fn a_constraint_on_a_non_bool_fails_the_run() {
    // The macro makes a constraint's function return a `bool`, so only a
    // hand-built rule can pass it anything else.
    let rules = RuleSet::build(|stage| {
        let pattern = double_neg_of_argument(stage).pattern();
        vec![
            PatternRule::new(pattern).check("is_block_argument", |_, _| {
                Ok(Some(vec![NativeValue::new(1_i64)]))
            }),
        ]
    })
    .unwrap();
    let input = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    let (mut pipeline, stage) = parse(&input);
    let driver = RewriteDriver::new(&rules);

    let mut failure = None;
    let result = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<PostOrder>(rewriter);
        failure = result.clone().err();
        result
    });

    assert!(result.is_err());
    assert_eq!(
        failure,
        Some(DriverError::ConstraintNotBool {
            rule: "double_neg_of_argument".to_string()
        })
    );
}

#[test]
fn a_native_argument_of_the_wrong_type_fails_the_run() {
    let rules = RuleSet::build(|stage| vec![double_neg_of_zero(stage)]).unwrap();
    let input = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    let (mut pipeline, stage) = parse(&input);
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
        Some(DriverError::NativeArgument {
            ref name,
            position: 0,
            expected: "i64",
            found: Some(_),
            ..
        }) if name == "is_zero"
    ));
}

#[test]
fn an_unregistered_native_function_rejects_the_rule() {
    // The macro registers every function a rule calls, so only a hand-built
    // rule can leave one out.
    let error = RuleSet::build(|stage| {
        vec![PatternRule::<L>::new(
            double_neg_of_argument(stage).pattern(),
        )]
    })
    .err()
    .expect("`is_block_argument` is not registered");
    assert_eq!(error.rule.as_deref(), Some("double_neg_of_argument"));
    assert_eq!(
        error.kind,
        RuleErrorKind::UnknownCheck("is_block_argument".to_string())
    );
}

/// The reverse BFS orders allow erasing the statement being visited.
#[test]
fn a_rule_erasing_its_root_runs_in_the_reverse_bfs_orders() {
    let rules = RuleSet::build(|stage| vec![double_neg_erasing_root(stage)]).unwrap();
    let input = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    let expected = printed(&program("", "%x"));
    assert_eq!(rewrite::<ReverseBFS>(&rules, &input).0, expected);
    assert_eq!(rewrite::<KeepOrderReverseBFS>(&rules, &input).0, expected);
}

/// The other orders forbid erasing the statement being visited, so the run
/// fails before editing anything.
#[test]
fn a_rule_erasing_its_root_fails_in_the_other_orders() {
    let rules = RuleSet::build(|stage| vec![double_neg_erasing_root(stage)]).unwrap();
    let input = program("    %a = neg %x -> i64;\n    %b = neg %a -> i64;", "%b");
    let (mut pipeline, stage) = parse(&input);
    let driver = RewriteDriver::new(&rules);

    let mut failure = None;
    let mut edits = None;
    let result = pipeline.run_pass(stage, |rewriter| {
        let result = driver.run::<PostOrder>(rewriter);
        failure = result.clone().err();
        edits = Some(rewriter.events().len());
        result
    });

    assert!(result.is_err());
    assert_eq!(
        failure,
        Some(DriverError::ErasesRoot {
            rule: "double_neg_erasing_root".to_string(),
            order: "PostOrder",
        })
    );
    assert_eq!(edits, Some(0));
}

#[test]
fn a_failed_rewrite_quarantines_the_stage() {
    // `%a` is also used by the `add`, so erasing it fails.
    let input = program(
        "    %a = neg %x -> i64;\n    %b = neg %a -> i64;\n    %s = add %a, %b -> i64;",
        "%s",
    );
    let (mut pipeline, stage) = parse(&input);
    let rules = RuleSet::build(|stage| vec![double_neg_erasing_producer(stage)]).unwrap();
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
