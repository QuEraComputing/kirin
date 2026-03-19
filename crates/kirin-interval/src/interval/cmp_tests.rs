use kirin_cmp::CompareValue;

use super::*;
use crate::BoolInterval;

#[test]
fn test_cmp_eq_equal_points() {
    let a = Interval::constant(5);
    let b = Interval::constant(5);
    assert_eq!(a.cmp_eq(&b), BoolInterval::True);
}

#[test]
fn test_cmp_eq_disjoint() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    assert_eq!(a.cmp_eq(&b), BoolInterval::False);
}

#[test]
fn test_cmp_eq_overlapping() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    assert_eq!(a.cmp_eq(&b), BoolInterval::Unknown);
}

#[test]
fn test_cmp_eq_bottom() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(0, 10);
    assert_eq!(bot.cmp_eq(&a), BoolInterval::Bottom);
    assert_eq!(a.cmp_eq(&bot), BoolInterval::Bottom);
}

#[test]
fn test_cmp_ne_equal_points() {
    let a = Interval::constant(5);
    assert_eq!(a.cmp_ne(&a), BoolInterval::False);
}

#[test]
fn test_cmp_ne_disjoint() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    assert_eq!(a.cmp_ne(&b), BoolInterval::True);
}

#[test]
fn test_cmp_ne_overlapping() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    assert_eq!(a.cmp_ne(&b), BoolInterval::Unknown);
}

#[test]
fn test_cmp_lt_definitely_true() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    assert_eq!(a.cmp_lt(&b), BoolInterval::True);
}

#[test]
fn test_cmp_lt_definitely_false() {
    let a = Interval::new(5, 10);
    let b = Interval::new(0, 3);
    assert_eq!(a.cmp_lt(&b), BoolInterval::False);
}

#[test]
fn test_cmp_lt_equal_point() {
    let a = Interval::constant(5);
    let b = Interval::constant(5);
    // b.hi(5) <= a.lo(5) -> false
    assert_eq!(a.cmp_lt(&b), BoolInterval::False);
}

#[test]
fn test_cmp_lt_overlapping() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    assert_eq!(a.cmp_lt(&b), BoolInterval::Unknown);
}

#[test]
fn test_cmp_le_touching() {
    let a = Interval::new(0, 5);
    let b = Interval::new(5, 10);
    // a.hi(5) <= b.lo(5) -> definitely true
    assert_eq!(a.cmp_le(&b), BoolInterval::True);
}

#[test]
fn test_cmp_le_reverse() {
    let a = Interval::new(5, 10);
    let b = Interval::new(0, 3);
    // b.hi(3) < a.lo(5) -> definitely false
    assert_eq!(a.cmp_le(&b), BoolInterval::False);
}

#[test]
fn test_cmp_gt_definitely_true() {
    let a = Interval::new(10, 20);
    let b = Interval::new(0, 5);
    assert_eq!(a.cmp_gt(&b), BoolInterval::True);
}

#[test]
fn test_cmp_gt_definitely_false() {
    let a = Interval::new(0, 5);
    let b = Interval::new(10, 20);
    assert_eq!(a.cmp_gt(&b), BoolInterval::False);
}

#[test]
fn test_cmp_ge_touching() {
    let a = Interval::new(5, 10);
    let b = Interval::new(0, 5);
    // b.hi(5) <= a.lo(5) -> definitely true
    assert_eq!(a.cmp_ge(&b), BoolInterval::True);
}

#[test]
fn test_cmp_ge_definitely_false() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    // a.hi(3) < b.lo(5) -> definitely false
    assert_eq!(a.cmp_ge(&b), BoolInterval::False);
}

#[test]
fn test_cmp_all_bottom_propagates() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(0, 10);
    assert_eq!(bot.cmp_lt(&a), BoolInterval::Bottom);
    assert_eq!(bot.cmp_le(&a), BoolInterval::Bottom);
    assert_eq!(bot.cmp_gt(&a), BoolInterval::Bottom);
    assert_eq!(bot.cmp_ge(&a), BoolInterval::Bottom);
    assert_eq!(a.cmp_lt(&bot), BoolInterval::Bottom);
    assert_eq!(a.cmp_le(&bot), BoolInterval::Bottom);
    assert_eq!(a.cmp_gt(&bot), BoolInterval::Bottom);
    assert_eq!(a.cmp_ge(&bot), BoolInterval::Bottom);
}

// --- Conversion roundtrip: BoolInterval -> Interval preserves semantics ---

#[test]
fn test_bool_interval_to_interval_consistency() {
    let a = Interval::constant(5);
    let b = Interval::constant(5);
    let result: Interval = a.cmp_eq(&b).into();
    assert_eq!(result, Interval::constant(1));

    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    let result: Interval = a.cmp_eq(&b).into();
    assert_eq!(result, Interval::constant(0));

    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    let result: Interval = a.cmp_eq(&b).into();
    assert_eq!(result, Interval::new(0, 1));
}
