use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_cmp::Cmp;
use kirin_test_utils::roundtrip;

fn assert_cmp_roundtrip(input: &str, operands: &[(&str, ArithType)]) {
    let (stage, statement) = roundtrip::emit_statement::<Cmp<ArithType>>(input, operands);

    // Verify dialect properties: all cmp operations are pure and speculatable
    let dialect = statement
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(dialect.is_pure(), "cmp statements should be pure");
    assert!(
        dialect.is_speculatable(),
        "cmp statements should be speculatable"
    );

    assert_eq!(
        roundtrip::render_statement::<Cmp<ArithType>>(&stage, statement).trim(),
        input
    );
}

#[test]
fn test_eq_roundtrip() {
    assert_cmp_roundtrip(
        "%res = eq %a, %b -> i64",
        &[("a", ArithType::I64), ("b", ArithType::I64)],
    );
}

#[test]
fn test_ne_roundtrip() {
    assert_cmp_roundtrip(
        "%res = ne %a, %b -> i32",
        &[("a", ArithType::I32), ("b", ArithType::I32)],
    );
}

#[test]
fn test_lt_roundtrip() {
    assert_cmp_roundtrip(
        "%res = lt %a, %b -> f64",
        &[("a", ArithType::F64), ("b", ArithType::F64)],
    );
}

#[test]
fn test_le_roundtrip() {
    assert_cmp_roundtrip(
        "%res = le %a, %b -> i64",
        &[("a", ArithType::I64), ("b", ArithType::I64)],
    );
}

#[test]
fn test_gt_roundtrip() {
    assert_cmp_roundtrip(
        "%res = gt %a, %b -> u32",
        &[("a", ArithType::U32), ("b", ArithType::U32)],
    );
}

#[test]
fn test_ge_roundtrip() {
    assert_cmp_roundtrip(
        "%res = ge %a, %b -> i64",
        &[("a", ArithType::I64), ("b", ArithType::I64)],
    );
}
