//! Building patterns programmatically in the generic [`PatternLanguage`].

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_test_languages::GraphFunctionLanguage as L;
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

#[test]
fn add_zero_built_on_generic_pattern_language() {
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
    // `rewrite` and `erase` name the root by its position in the body below.
    let root_position = 2;
    let replace = Replace::<ArithType>::new(&mut stage, r, x.result);
    let erase = Erase::<ArithType>::new(&mut stage, root_position);
    let rw = stage.block().name("rw").stmt(replace).stmt(erase).new();
    let rewrite = Rewrite::<ArithType>::new(&mut stage, root_position, rw);

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

    let PatternLanguage::Wildcard(PatternStatement::Pattern(root)) =
        pattern.id.expect_info(&stage).definition()
    else {
        panic!("the top-level statement should be the pattern");
    };
    assert_eq!(root.body(), bb);
    assert_eq!(bb.statements(&stage).nth(root_position), Some(r_stmt));

    let PatternLanguage::Op(L::Arith(Arith::Add { lhs, rhs, .. })) =
        r_stmt.expect_info(&stage).definition()
    else {
        panic!("the root should be `L`'s own `add`");
    };
    assert_eq!(*lhs, SSAValue::from(x.result));
    assert_eq!(*rhs, SSAValue::from(zero));
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
