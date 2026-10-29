use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_test_utils::roundtrip;
use kirin_wildcard::{Erase, Hole, Pattern, PatternStatement, Replace, Rewrite};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum PatternLanguage {
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    // `lift_project_from` adds `From<Hole<ArithType>>` etc. through
    // `PatternStatement`, which the generated `Hole::new`-style builders require.
    #[wraps(lift_project_from(
        Pattern<ArithType>,
        Hole<ArithType>,
        Replace<ArithType>,
        Erase<ArithType>,
        Rewrite<ArithType>
    ))]
    Wildcard(PatternStatement<ArithType>),
}

#[test]
fn test_hole_roundtrip() {
    roundtrip::assert_statement_roundtrip::<PatternLanguage>("%x = wildcard.hole -> i64", &[]);
}

#[test]
fn test_native_call_roundtrip() {
    roundtrip::assert_statement_roundtrip::<PatternLanguage>(
        "%k = wildcard.native_call \"log2\"(%c) -> i64",
        &[("c", ArithType::I64)],
    );
}

#[test]
fn test_native_call_multiple_results_roundtrip() {
    roundtrip::assert_statement_roundtrip::<PatternLanguage>(
        "%q, %r = wildcard.native_call \"divmod\"(%x, %y) -> i64, i64",
        &[("x", ArithType::I64), ("y", ArithType::I64)],
    );
}

#[test]
fn test_native_call_without_results_roundtrip() {
    roundtrip::assert_statement_roundtrip::<PatternLanguage>(
        "wildcard.native_call \"is_power_of_two\"(%c)",
        &[("c", ArithType::I64)],
    );
}

#[test]
fn test_replace_roundtrip() {
    roundtrip::assert_statement_roundtrip::<PatternLanguage>(
        "wildcard.replace %r with %x",
        &[("r", ArithType::I64), ("x", ArithType::I64)],
    );
}

#[test]
fn test_erase_roundtrip() {
    roundtrip::assert_statement_roundtrip::<PatternLanguage>("wildcard.erase 2", &[]);
}

#[test]
fn test_add_zero_pattern_roundtrip() {
    let input = r#"
wildcard.pattern @add_zero block ^bb() {
  %x = wildcard.hole -> i64;
  %zero = constant 0 -> i64;
  %r = add %x, %zero -> i64;
  wildcard.rewrite 2 block ^rw() {
    wildcard.replace %r with %x;
    wildcard.erase 2;
  };
}"#;
    roundtrip::assert_statement_roundtrip::<PatternLanguage>(input.trim(), &[]);
}

#[test]
fn test_add_zero_pattern_built_with_builder() {
    let mut stage: BuilderStageInfo<PatternLanguage> = BuilderStageInfo::default();

    // Match: a hole, the zero constant, and the root `add`.
    let x = Hole::<ArithType>::new(&mut stage);
    let zero = Constant::<ArithValue, ArithType>::new(&mut stage, ArithValue::I64(0));
    let r = Arith::<ArithType>::op_add(&mut stage, x.result, zero.result);

    // Replacement: redirect uses of the root to the hole, then erase the root,
    // which is at position 2 of the body below.
    let replace = Replace::<ArithType>::new(&mut stage, r.result, x.result);
    let erase = Erase::<ArithType>::new(&mut stage, 2);
    let rw = stage.block().name("rw").stmt(replace).stmt(erase).new();
    let rewrite = Rewrite::<ArithType>::new(&mut stage, 2, rw);

    let bb = stage
        .block()
        .name("bb")
        .stmt(x)
        .stmt(zero)
        .stmt(r)
        .terminator(rewrite)
        .new();
    let name = stage.symbol_table_mut().intern("add_zero".to_string());
    let pattern = Pattern::<ArithType>::new(&mut stage, name, bb);

    let stage = stage.finalize().expect("pattern should finalize");
    let rendered = roundtrip::render_statement(&stage, pattern.id);
    // Builder-made values have no names, so they print by id.
    let expected = r#"
wildcard.pattern @add_zero block ^bb {
  %0 = wildcard.hole -> i64;
  %1 = constant 0 -> i64;
  %2 = add %0, %1 -> i64;
  wildcard.rewrite 2 block ^rw {
    wildcard.replace %2 with %0;
    wildcard.erase 2;
  };
}"#;
    assert_eq!(rendered.trim(), expected.trim());
    roundtrip::assert_statement_roundtrip::<PatternLanguage>(rendered.trim(), &[]);
}
