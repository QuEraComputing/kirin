use kirin_interpreter::BranchCondition;

use super::*;

#[test]
fn test_is_truthy_all_positive() {
    let a = Interval::new(1, 10);
    assert_eq!(a.is_truthy(), Some(true));
}

#[test]
fn test_is_truthy_all_negative() {
    let a = Interval::new(-10, -1);
    assert_eq!(a.is_truthy(), Some(true));
}

#[test]
fn test_is_truthy_zero() {
    let a = Interval::constant(0);
    assert_eq!(a.is_truthy(), Some(false));
}

#[test]
fn test_is_truthy_straddles_zero() {
    let a = Interval::new(-5, 5);
    assert_eq!(a.is_truthy(), None);
}

#[test]
fn test_is_truthy_bottom() {
    let bot = Interval::bottom_interval();
    assert_eq!(bot.is_truthy(), None);
}

#[test]
fn test_is_truthy_includes_zero_and_positive() {
    let a = Interval::new(0, 10);
    assert_eq!(a.is_truthy(), None);
}

#[test]
fn test_is_truthy_single_positive() {
    let a = Interval::constant(42);
    assert_eq!(a.is_truthy(), Some(true));
}

#[test]
fn test_is_truthy_single_negative() {
    let a = Interval::constant(-1);
    assert_eq!(a.is_truthy(), Some(true));
}

#[test]
fn test_is_truthy_half_bounded_positive() {
    let a = Interval::half_bounded_below(1); // [1, +inf)
    assert_eq!(a.is_truthy(), Some(true));
}

#[test]
fn test_is_truthy_half_bounded_negative() {
    let a = Interval::half_bounded_above(-1); // (-inf, -1]
    assert_eq!(a.is_truthy(), Some(true));
}

#[test]
fn test_is_truthy_half_bounded_includes_zero() {
    let a = Interval::half_bounded_below(0); // [0, +inf)
    assert_eq!(a.is_truthy(), None);
}
