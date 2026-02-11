use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_test_utils::roundtrip;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[wraps]
#[kirin(fn, type = ArithType)]
enum NumericLanguage {
    Arith(Arith<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    ControlFlow(ControlFlow<ArithType>),
}

fn assert_roundtrip(input: &str, operands: &[(&str, ArithType)], speculatable: bool) {
    let (stage, statement) = roundtrip::emit_statement::<Arith<ArithType>>(input, operands);
    let dialect = statement
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(dialect.is_pure(), "arith statements should be pure");
    assert_eq!(
        dialect.is_speculatable(),
        speculatable,
        "unexpected speculatability for '{}'",
        input
    );
    assert_eq!(
        roundtrip::render_statement::<Arith<ArithType>>(&stage, statement).trim(),
        input
    );
}

#[test]
fn test_roundtrip_all_operations_with_integer_types() {
    assert_roundtrip(
        "%ri_add = add %a, %b -> i32",
        &[("a", ArithType::I32), ("b", ArithType::I32)],
        true,
    );
    assert_roundtrip(
        "%ri_sub = sub %a, %b -> i64",
        &[("a", ArithType::I64), ("b", ArithType::I64)],
        true,
    );
    assert_roundtrip(
        "%ri_mul = mul %a, %b -> u32",
        &[("a", ArithType::U32), ("b", ArithType::U32)],
        true,
    );
    assert_roundtrip(
        "%ri_div = div %a, %b -> i128",
        &[("a", ArithType::I128), ("b", ArithType::I128)],
        false,
    );
    assert_roundtrip(
        "%ri_rem = rem %a, %b -> u64",
        &[("a", ArithType::U64), ("b", ArithType::U64)],
        false,
    );
    assert_roundtrip("%ri_neg = neg %a -> i16", &[("a", ArithType::I16)], true);
}

#[test]
fn test_roundtrip_all_operations_with_float_types() {
    assert_roundtrip(
        "%rf_add = add %x, %y -> f64",
        &[("x", ArithType::F64), ("y", ArithType::F64)],
        true,
    );
    assert_roundtrip(
        "%rf_sub = sub %x, %y -> f32",
        &[("x", ArithType::F32), ("y", ArithType::F32)],
        true,
    );
    assert_roundtrip(
        "%rf_mul = mul %x, %y -> f64",
        &[("x", ArithType::F64), ("y", ArithType::F64)],
        true,
    );
    assert_roundtrip(
        "%rf_div = div %x, %y -> f32",
        &[("x", ArithType::F32), ("y", ArithType::F32)],
        false,
    );
    assert_roundtrip(
        "%rf_rem = rem %x, %y -> f64",
        &[("x", ArithType::F64), ("y", ArithType::F64)],
        false,
    );
    assert_roundtrip("%rf_neg = neg %x -> f32", &[("x", ArithType::F32)], true);
}

#[test]
fn test_composes_with_constant_and_control_flow() {
    let mut stage: StageInfo<NumericLanguage> = StageInfo::default();
    let const_a = Constant::<ArithValue, ArithType>::new(&mut stage, ArithValue::I32(1));
    let const_b = Constant::<ArithValue, ArithType>::new(&mut stage, ArithValue::I32(2));
    let add_stmt = Arith::<ArithType>::op_add(&mut stage, const_a.result, const_b.result);
    let ret_stmt = ControlFlow::<ArithType>::op_return(&mut stage, add_stmt.result);

    let const_a_def = const_a
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(const_a_def, NumericLanguage::Constant(_)),
        "expected wrapped constant statement"
    );

    let const_b_def = const_b
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(const_b_def, NumericLanguage::Constant(_)),
        "expected wrapped constant statement"
    );

    let add_def = add_stmt
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(add_def, NumericLanguage::Arith(Arith::Add { .. })),
        "expected wrapped arith::add statement"
    );
    assert!(add_def.is_pure(), "arith add should remain pure");
    assert!(
        add_def.is_speculatable(),
        "arith add should remain speculatable"
    );

    let ret_def = ret_stmt
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(
            ret_def,
            NumericLanguage::ControlFlow(ControlFlow::Return(_))
        ),
        "expected wrapped cf::ret statement"
    );
    assert!(
        ret_def.is_terminator(),
        "cf::ret should remain a terminator"
    );
}
