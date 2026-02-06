//! Tests for enum-based dialect definitions with various field patterns.

use kirin_derive::Dialect;
use kirin_ir::*;
use kirin_test_utils::*;

#[derive(Dialect, Clone, Debug, PartialEq)]
#[kirin(fn, type = SimpleIRType, crate = kirin_ir)]
enum EnumDialect {
    Named { arg: SSAValue },
    Tuple(SSAValue, SSAValue),
    Unit,
}

#[test]
fn test_enum_named_variant() {
    let v1: SSAValue = TestSSAValue(1).into();
    let one = EnumDialect::Named { arg: v1 };

    assert_eq!(one.arguments().count(), 1);
    assert_eq!(one.arguments().next(), Some(&v1));
}

#[test]
fn test_enum_tuple_variant() {
    let v1: SSAValue = TestSSAValue(1).into();
    let v2: SSAValue = TestSSAValue(2).into();
    let two = EnumDialect::Tuple(v1, v2);

    let args: Vec<_> = two.arguments().cloned().collect();
    assert_eq!(args, vec![v1, v2]);
}

#[test]
fn test_enum_unit_variant() {
    let unit = EnumDialect::Unit;
    assert_eq!(unit.arguments().count(), 0);
}
