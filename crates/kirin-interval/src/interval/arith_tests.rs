use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_ir::HasTop;

use super::*;

#[test]
fn test_checked_div_returns_top() {
    let a = Interval::new(1, 10);
    let b = Interval::new(2, 5);
    let result = a.checked_div(b);
    assert_eq!(result, Some(Interval::top()));
}

#[test]
fn test_checked_rem_returns_top() {
    let a = Interval::new(1, 10);
    let b = Interval::new(2, 5);
    let result = a.checked_rem(b);
    assert_eq!(result, Some(Interval::top()));
}

#[test]
fn test_from_arith_value_i64() {
    let v = ArithValue::I64(42);
    let interval: Interval = v.into();
    assert_eq!(interval, Interval::constant(42));
}

#[test]
fn test_from_arith_value_i32() {
    let v = ArithValue::I32(-7);
    let interval: Interval = v.into();
    assert_eq!(interval, Interval::constant(-7));
}

#[test]
fn test_from_arith_value_i16() {
    let v = ArithValue::I16(100);
    let interval: Interval = v.into();
    assert_eq!(interval, Interval::constant(100));
}

#[test]
fn test_from_arith_value_i8() {
    let v = ArithValue::I8(-1);
    let interval: Interval = v.into();
    assert_eq!(interval, Interval::constant(-1));
}

#[test]
fn test_from_arith_value_float_becomes_top() {
    let v = ArithValue::F64(2.72);
    let interval: Interval = v.into();
    assert_eq!(interval, Interval::top());

    let v = ArithValue::F32(1.0);
    let interval: Interval = v.into();
    assert_eq!(interval, Interval::top());
}
