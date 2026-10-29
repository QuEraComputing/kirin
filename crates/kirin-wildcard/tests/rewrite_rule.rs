//! `#[rewrite_rule]` end to end: each rule expands into a function building
//! a valid pattern into a `PatternLanguage<L>` stage it is given.

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_test_languages::{EffectLanguage, GraphFunctionLanguage as L};
use kirin_test_utils::roundtrip;
use kirin_wildcard::{PatternLanguage, PatternStatement, Rewrite, rewrite_rule};

type Stage = StageInfo<PatternLanguage<L>>;

/// Constraint and computation functions only have to exist: the generated
/// code names them so rustc checks it, but never calls them.
fn is_power_of_two(x: i64) -> bool {
    x > 0 && (x & (x - 1)) == 0
}

mod native {
    pub fn build_neg(x: i64) -> i64 {
        -x
    }
}

#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    is_power_of_two(x);
    rewrite!(r, {
        replace!(r, x);
        erase!(r);
    })
}

/// Whether `x` has no uses; dead-code elimination guards its `erase!` with
/// it, since the results are not replaced.
fn has_no_uses(_x: i64) -> bool {
    unimplemented!("only named by the rule")
}

/// Dead-code elimination: erase a `neg` whose result is unused, without
/// replacing it.
#[rewrite_rule(language = L)]
fn dead_neg() {
    let r = op![Arith::neg(x)];
    has_no_uses(r);
    erase!(r)
}

/// An op without results, named with `@ ()`, as the root and erased.
#[rewrite_rule(language = EffectLanguage)]
fn drop_effect_of_neg() {
    let n = op![Arith::neg(x)];
    let k @ () = op![EffectLanguage::effect(n)];
    erase!(k)
}

/// A terminator of the language as the root: an `exit` of a double negation
/// exits with the value itself.
#[rewrite_rule(language = EffectLanguage)]
fn exit_double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    let k @ () = op![EffectLanguage::exit(r)];
    rewrite!(k, {
        replace!(r, x);
    })
}

mod rules {
    use super::{Arith, L, native};
    use kirin_wildcard::{self as wildcard, rewrite_rule};

    /// Visibility, an explicit crate path, an anonymous hole and a native
    /// function building the replacement.
    #[rewrite_rule(language = L, crate = wildcard)]
    pub fn add_any() {
        let r = op![Arith::add(x, _)];
        rewrite!(r, {
            let n = native::build_neg(x);
            replace!(r, n);
        })
    }
}

/// The name `value` was given in the rule, or `None` for `_`.
fn name(stage: &Stage, value: impl Into<SSAValue>) -> Option<String> {
    let symbol = value.into().expect_info(stage).name()?;
    stage.symbol_table().resolve(symbol).cloned()
}

/// A short description of `stmt`, naming its results.
fn describe(stage: &Stage, stmt: Statement) -> String {
    let definition = stmt.expect_info(stage).definition();
    let results: Vec<String> = definition
        .results()
        .map(|&r| name(stage, r).unwrap_or_else(|| "_".to_string()))
        .collect();
    let kind = match definition {
        PatternLanguage::Op(L::Arith(Arith::Neg { .. })) => "neg".to_string(),
        PatternLanguage::Op(L::Arith(Arith::Add { .. })) => "add".to_string(),
        PatternLanguage::Wildcard(PatternStatement::Hole(_)) => "hole".to_string(),
        PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) => {
            format!("call {}", call.name())
        }
        PatternLanguage::Wildcard(PatternStatement::Constraint(_)) => "constraint".to_string(),
        PatternLanguage::Wildcard(PatternStatement::Replace(_)) => "replace".to_string(),
        PatternLanguage::Wildcard(PatternStatement::Erase(_)) => "erase".to_string(),
        other => panic!("unexpected statement {other:?}"),
    };
    if results.is_empty() {
        kind
    } else {
        format!("{} = {kind}", results.join(", "))
    }
}

/// Build `rule`'s pattern in a stage of its own, and finalize it.
fn build<Lang: Dialect>(
    rule: impl FnOnce(&mut BuilderStageInfo<PatternLanguage<Lang>>) -> Statement,
) -> (StageInfo<PatternLanguage<Lang>>, Statement) {
    let mut stage = BuilderStageInfo::default();
    let pattern = rule(&mut stage);
    let stage = stage.finalize().expect("the rule builds a valid pattern");
    (stage, pattern)
}

/// The `wildcard.pattern` statement `pattern`'s name and body, and its
/// `wildcard.rewrite` terminator.
fn pattern_parts<Lang: Dialect<Type = ArithType>>(
    stage: &StageInfo<PatternLanguage<Lang>>,
    pattern: Statement,
) -> (String, Vec<Statement>, Rewrite<ArithType>) {
    let PatternLanguage::Wildcard(PatternStatement::Pattern(pattern)) =
        pattern.expect_info(stage).definition()
    else {
        panic!("expected a `wildcard.pattern`");
    };
    let name = stage
        .symbol_table()
        .resolve(pattern.name())
        .cloned()
        .unwrap();
    let body = pattern.body();
    let statements = body.statements(stage).collect();
    let terminator = body
        .terminator(stage)
        .expect("a pattern ends in its rewrite");
    let PatternLanguage::Wildcard(PatternStatement::Rewrite(rewrite)) =
        terminator.expect_info(stage).definition()
    else {
        panic!("expected the rewrite as the pattern's terminator");
    };
    (name, statements, rewrite.clone())
}

#[test]
fn rewrite_rule_builds_the_match_and_the_rewrite() {
    let (stage, pattern) = build(double_neg);
    let (name_, statements, rewrite) = pattern_parts(&stage, pattern);
    assert_eq!(name_, "double_neg");

    let described: Vec<String> = statements.iter().map(|&s| describe(&stage, s)).collect();
    assert_eq!(
        described,
        [
            "x = hole",
            "a = neg",
            "r = neg",
            "_ = call is_power_of_two",
            "constraint",
        ]
    );

    // The root is `r`, and the rewrite redirects its uses to `x`, then
    // erases it.
    let root = rewrite.root();
    assert_eq!(root, statements[2]);
    assert_eq!(describe(&stage, root), "r = neg");
    let actions: Vec<Statement> = rewrite.body().statements(&stage).collect();
    let PatternLanguage::Wildcard(PatternStatement::Replace(replace)) =
        actions[0].expect_info(&stage).definition()
    else {
        panic!("expected a replace");
    };
    let r = root
        .expect_info(&stage)
        .definition()
        .results()
        .next()
        .copied();
    assert_eq!(Some(replace.matched()), r.map(SSAValue::from));
    assert_eq!(name(&stage, replace.replacement()).as_deref(), Some("x"));
    let PatternLanguage::Wildcard(PatternStatement::Erase(erase)) =
        actions[1].expect_info(&stage).definition()
    else {
        panic!("expected an erase");
    };
    assert_eq!(erase.target(), rewrite.root());
}

#[test]
fn rewrite_rule_erases_without_replacing() {
    let (stage, pattern) = build(dead_neg);
    let (name_, statements, rewrite) = pattern_parts(&stage, pattern);
    assert_eq!(name_, "dead_neg");

    let described: Vec<String> = statements.iter().map(|&s| describe(&stage, s)).collect();
    assert_eq!(
        described,
        ["x = hole", "r = neg", "_ = call has_no_uses", "constraint"]
    );

    // The rewrite only erases the root.
    assert_eq!(rewrite.root(), statements[1]);
    let actions: Vec<Statement> = rewrite.body().statements(&stage).collect();
    let [action] = actions[..] else {
        panic!("expected one action");
    };
    let PatternLanguage::Wildcard(PatternStatement::Erase(erase)) =
        action.expect_info(&stage).definition()
    else {
        panic!("expected an erase");
    };
    assert_eq!(erase.target(), rewrite.root());
}

#[test]
fn rewrite_rule_roots_and_erases_an_op_without_results() {
    let (stage, pattern) = build(drop_effect_of_neg);
    let (name_, statements, rewrite) = pattern_parts(&stage, pattern);
    assert_eq!(name_, "drop_effect_of_neg");

    // Match: the hole `x`, the `neg`, and the `effect` consuming it.
    assert_eq!(statements.len(), 3);
    let root = rewrite.root();
    assert_eq!(root, statements[2]);
    let PatternLanguage::Op(EffectLanguage::Effect { value }) =
        root.expect_info(&stage).definition()
    else {
        panic!("expected the effect as the root");
    };
    assert_eq!(root.expect_info(&stage).definition().results().count(), 0);
    let neg = statements[1].expect_info(&stage).definition();
    assert_eq!(
        neg.results().next().copied().map(SSAValue::from),
        Some(*value)
    );

    // The lone `erase!` erases the root itself.
    let actions: Vec<Statement> = rewrite.body().statements(&stage).collect();
    let [action] = actions[..] else {
        panic!("expected one action");
    };
    let PatternLanguage::Wildcard(PatternStatement::Erase(erase)) =
        action.expect_info(&stage).definition()
    else {
        panic!("expected an erase");
    };
    assert_eq!(erase.target(), rewrite.root());
}

/// A terminator of the language is an ordinary statement of the pattern body,
/// which the `rewrite` still ends.
#[test]
fn rewrite_rule_roots_at_a_terminator_of_the_language() {
    let (stage, pattern) = build(exit_double_neg);
    let (_, statements, rewrite) = pattern_parts(&stage, pattern);

    // Match: the hole `x`, both `neg`s, and the `exit` consuming the second.
    assert_eq!(statements.len(), 4);
    let root = rewrite.root();
    assert_eq!(root, statements[3]);
    let definition = root.expect_info(&stage).definition();
    let PatternLanguage::Op(exit @ EffectLanguage::Exit { .. }) = definition else {
        panic!("expected the exit as the root");
    };
    assert!(exit.is_terminator());
    assert!(!definition.is_terminator());
}

/// The printed rule shows the root, in `rewrite` and `erase`, by its result.
#[test]
fn rewrite_rule_prints_the_root_by_its_result() {
    let (stage, pattern) = build(double_neg);
    let expected = r#"
wildcard.pattern @double_neg block ^double_neg_pat {
  %x = wildcard.hole -> i64;
  %a = neg %x -> i64;
  %r = neg %a -> i64;
  %3 = wildcard.native_call "is_power_of_two" (%x) -> i64;
  wildcard.constraint %3;
  wildcard.rewrite %r block ^double_neg_rw {
    wildcard.replace %r with %x;
    wildcard.erase %r;
  };
}
"#;
    assert_eq!(
        roundtrip::render_statement(&stage, pattern).trim(),
        expected.trim()
    );
}

/// The printed rule shows a root without results quoted, followed by its
/// position in the pattern body.
#[test]
fn rewrite_rule_prints_a_root_without_results_quoted_with_its_position() {
    let (stage, pattern) = build(drop_effect_of_neg);
    let expected = r#"
wildcard.pattern @drop_effect_of_neg block ^drop_effect_of_neg_pat {
  %x = wildcard.hole -> i64;
  %n = neg %x -> i64;
  effect %n;
  wildcard.rewrite 'effect %n' (#2) block ^drop_effect_of_neg_rw {
    wildcard.erase 'effect %n' (#2);
  };
}
"#;
    assert_eq!(
        roundtrip::render_statement(&stage, pattern).trim(),
        expected.trim()
    );
}

/// The printed rule shows a terminator of the language in the pattern body,
/// before the `rewrite` ending it.
#[test]
fn rewrite_rule_prints_a_terminator_of_the_language_in_the_body() {
    let (stage, pattern) = build(exit_double_neg);
    let expected = r#"
wildcard.pattern @exit_double_neg block ^exit_double_neg_pat {
  %x = wildcard.hole -> i64;
  %a = neg %x -> i64;
  %r = neg %a -> i64;
  exit %r;
  wildcard.rewrite 'exit %r' (#3) block ^exit_double_neg_rw {
    wildcard.replace %r with %x;
  };
}
"#;
    assert_eq!(
        roundtrip::render_statement(&stage, pattern).trim(),
        expected.trim()
    );
}

#[test]
fn rewrite_rule_ties_the_constraint_to_its_call() {
    let (stage, pattern) = build(double_neg);
    let (_, statements, _) = pattern_parts(&stage, pattern);

    let PatternLanguage::Wildcard(PatternStatement::NativeCall(call)) =
        statements[3].expect_info(&stage).definition()
    else {
        panic!("expected the constraint's native call");
    };
    let [x] = call.args() else {
        panic!("expected one argument");
    };
    assert_eq!(name(&stage, *x).as_deref(), Some("x"));

    let PatternLanguage::Wildcard(PatternStatement::Constraint(constraint)) =
        statements[4].expect_info(&stage).definition()
    else {
        panic!("expected the constraint");
    };
    let success = statements[3]
        .expect_info(&stage)
        .definition()
        .results()
        .next()
        .copied()
        .unwrap();
    assert_eq!(constraint.success(), SSAValue::from(success));
}

#[test]
fn rewrite_rule_keeps_visibility_and_takes_a_crate_path() {
    let (stage, pattern) = build(rules::add_any);
    let (name_, statements, rewrite) = pattern_parts(&stage, pattern);
    assert_eq!(name_, "add_any");

    // `_` is a hole of its own, with no name.
    let described: Vec<String> = statements.iter().map(|&s| describe(&stage, s)).collect();
    assert_eq!(described, ["x = hole", "_ = hole", "r = add"]);

    // The native function is stored under its path as written, and its
    // result replaces the root.
    let actions: Vec<String> = rewrite
        .body()
        .statements(&stage)
        .map(|s| describe(&stage, s))
        .collect();
    assert_eq!(actions, ["n = call native::build_neg", "replace"]);
}

/// Several rules build into one stage, finalized once, and a finalized stage
/// can take more. Each pattern keeps its own name, body and root.
#[test]
fn rewrite_rules_share_one_stage() {
    let mut stage = BuilderStageInfo::<PatternLanguage<L>>::default();
    let first = double_neg(&mut stage);
    let second = dead_neg(&mut stage);
    let mut stage = stage.finalize().expect("both rules build valid patterns");
    let third = stage.with_builder(rules::add_any);

    let roots: Vec<(String, Statement)> = [first, second, third]
        .into_iter()
        .map(|pattern| {
            let (name, _, rewrite) = pattern_parts(&stage, pattern);
            (name, rewrite.root())
        })
        .collect();
    let names: Vec<&str> = roots.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["double_neg", "dead_neg", "add_any"]);
    let described: Vec<String> = roots
        .iter()
        .map(|&(_, root)| describe(&stage, root))
        .collect();
    assert_eq!(described, ["r = neg", "r = neg", "r = add"]);
    assert_ne!(roots[0].1, roots[1].1);
}
