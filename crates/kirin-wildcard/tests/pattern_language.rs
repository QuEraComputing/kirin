//! Building patterns programmatically in the generic [`PatternLanguage`].

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_test_utils::roundtrip;
use kirin_wildcard::{Erase, Hole, Pattern, PatternLanguage, PatternStatement, Replace, Rewrite};

type P = PatternLanguage<L>;

/// Add a statement of `L` with one named, typed result.
///
/// `L`'s generated builders (`Arith::op_add`, ...) cannot target
/// `PatternLanguage<L>`, so the result is allocated before the statement it
/// belongs to, the same way those builders do it internally.
fn op(
    stage: &mut BuilderStageInfo<P>,
    name: &str,
    ty: ArithType,
    make: impl FnOnce(ResultValue) -> L,
) -> (Statement, ResultValue) {
    let statement = stage.statement_arena().next_id();
    let result: ResultValue = stage
        .ssa()
        .name(name)
        .ty(ty)
        .kind(BuilderSSAKind::Result(statement, 0))
        .new()
        .into();
    stage
        .statement()
        .definition(PatternLanguage::Op(make(result)))
        .new();
    (statement, result)
}

/// The add-zero pattern built by [`add_zero`]: the pattern statement, its root
/// `add`, and the values `x` and `zero` the root adds.
struct AddZero {
    pattern: Statement,
    root: Statement,
    x: ResultValue,
    zero: ResultValue,
}

/// Build the pattern rewriting `x + 0` to `x`, and finalize its stage.
fn add_zero() -> (StageInfo<P>, AddZero) {
    let mut stage = BuilderStageInfo::<P>::default();

    // Match: a hole, the zero constant, and the root `add`.
    let x = Hole::<ArithType>::new(&mut stage);
    let (zero_stmt, zero) = op(&mut stage, "zero", ArithType::I64, |result| {
        L::Constant(Constant {
            value: ArithValue::I64(0),
            result,
            marker: std::marker::PhantomData,
        })
    });
    let (r_stmt, r) = op(&mut stage, "r", ArithType::I64, |result| {
        L::Arith(Arith::Add {
            lhs: x.result.into(),
            rhs: zero.into(),
            result,
        })
    });

    // Replacement: redirect uses of the root to the hole, then erase the root.
    let replace = Replace::<ArithType>::new(&mut stage, r, x.result);
    let erase = Erase::<ArithType>::new(&mut stage, r_stmt);
    let rw = stage.block().name("rw").stmt(replace).stmt(erase).new();
    let rewrite = Rewrite::<ArithType>::new(&mut stage, r_stmt, rw);

    let bb = stage
        .block()
        .name("bb")
        .stmt(x.id)
        .stmt(zero_stmt)
        .stmt(r_stmt)
        .terminator(rewrite)
        .new();
    let name = stage.symbol_table_mut().intern("add_zero".to_string());
    let pattern = Pattern::<ArithType>::new(&mut stage, name, bb);

    let stage = stage.finalize().expect("pattern should finalize");
    let add_zero = AddZero {
        pattern: pattern.id,
        root: r_stmt,
        x: x.result,
        zero,
    };
    (stage, add_zero)
}

#[test]
fn add_zero_built_on_generic_pattern_language() {
    let (stage, add_zero) = add_zero();

    let PatternLanguage::Wildcard(PatternStatement::Pattern(pattern)) =
        add_zero.pattern.expect_info(&stage).definition()
    else {
        panic!("the top-level statement should be the pattern");
    };
    assert_eq!(pattern.root(&stage), Some(add_zero.root));

    let PatternLanguage::Op(L::Arith(Arith::Add { lhs, rhs, .. })) =
        add_zero.root.expect_info(&stage).definition()
    else {
        panic!("the root should be `L`'s own `add`");
    };
    assert_eq!(*lhs, SSAValue::from(add_zero.x));
    assert_eq!(*rhs, SSAValue::from(add_zero.zero));
}

/// `rewrite` and `erase` print the root op by its result.
#[test]
fn add_zero_prints_the_root_by_its_result() {
    let (stage, add_zero) = add_zero();
    let rendered = roundtrip::render_statement(&stage, add_zero.pattern);
    // The hole has no name, so it prints by id.
    let expected = r#"
wildcard.pattern @add_zero block ^bb {
  %0 = wildcard.hole -> i64;
  %zero = constant 0 -> i64;
  %r = add %0, %zero -> i64;
  wildcard.rewrite %r block ^rw {
    wildcard.replace %r with %0;
    wildcard.erase %r;
  };
}"#;
    assert_eq!(rendered.trim(), expected.trim());
}

/// `new_add` builds `L`'s statement without a stage, so it can go into
/// `PatternLanguage<L>` through `L`'s own `From<Arith<_>>`, which also infers
/// `Arith`'s type parameter. This is the shape `#[rewrite_rule]` generates.
#[test]
fn op_built_with_generated_constructor() {
    let mut stage = BuilderStageInfo::<P>::default();
    let x = Hole::<ArithType>::new(&mut stage);
    let y = Hole::<ArithType>::new(&mut stage);

    let statement = stage.statement_arena().next_id();
    let r: ResultValue = stage
        .ssa()
        .name("r")
        .ty(ArithType::I64)
        .kind(BuilderSSAKind::Result(statement, 0))
        .new()
        .into();
    stage
        .statement()
        .definition(PatternLanguage::Op(<L as From<_>>::from(Arith::new_add(
            x.result, y.result, r,
        ))))
        .new();
    let stage = stage.finalize().expect("pattern should finalize");

    let PatternLanguage::Op(L::Arith(Arith::Add { lhs, rhs, result })) =
        statement.expect_info(&stage).definition()
    else {
        panic!("expected `L`'s own `add`");
    };
    assert_eq!(*lhs, SSAValue::from(x.result));
    assert_eq!(*rhs, SSAValue::from(y.result));
    assert_eq!(*result, r);
}
