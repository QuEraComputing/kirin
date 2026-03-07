use kirin_cmp::CompareValue;

use super::*;

#[test]
fn test_cmp_eq_equal_points() {
    let a = Interval::constant(5);
    let b = Interval::constant(5);
    assert_eq!(a.cmp_eq(&b), Interval::constant(1));
}

#[test]
fn test_cmp_eq_disjoint() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    assert_eq!(a.cmp_eq(&b), Interval::constant(0));
}

#[test]
fn test_cmp_eq_overlapping() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    assert_eq!(a.cmp_eq(&b), Interval::new(0, 1));
}

#[test]
fn test_cmp_eq_bottom() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(0, 10);
    assert!(bot.cmp_eq(&a).is_empty());
    assert!(a.cmp_eq(&bot).is_empty());
}

#[test]
fn test_cmp_ne_equal_points() {
    let a = Interval::constant(5);
    assert_eq!(a.cmp_ne(&a), Interval::constant(0));
}

#[test]
fn test_cmp_ne_disjoint() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    assert_eq!(a.cmp_ne(&b), Interval::constant(1));
}

#[test]
fn test_cmp_ne_overlapping() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    assert_eq!(a.cmp_ne(&b), Interval::new(0, 1));
}

#[test]
fn test_cmp_lt_definitely_true() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    assert_eq!(a.cmp_lt(&b), Interval::constant(1));
}

#[test]
fn test_cmp_lt_definitely_false() {
    let a = Interval::new(5, 10);
    let b = Interval::new(0, 3);
    assert_eq!(a.cmp_lt(&b), Interval::constant(0));
}

#[test]
fn test_cmp_lt_equal_point() {
    let a = Interval::constant(5);
    let b = Interval::constant(5);
    // b.hi(5) <= a.lo(5) -> false
    assert_eq!(a.cmp_lt(&b), Interval::constant(0));
}

#[test]
fn test_cmp_lt_overlapping() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 15);
    assert_eq!(a.cmp_lt(&b), Interval::new(0, 1));
}

#[test]
fn test_cmp_le_touching() {
    let a = Interval::new(0, 5);
    let b = Interval::new(5, 10);
    // a.hi(5) <= b.lo(5) -> definitely true
    assert_eq!(a.cmp_le(&b), Interval::constant(1));
}

#[test]
fn test_cmp_le_reverse() {
    let a = Interval::new(5, 10);
    let b = Interval::new(0, 3);
    // b.hi(3) < a.lo(5) -> definitely false
    assert_eq!(a.cmp_le(&b), Interval::constant(0));
}

#[test]
fn test_cmp_gt_definitely_true() {
    let a = Interval::new(10, 20);
    let b = Interval::new(0, 5);
    assert_eq!(a.cmp_gt(&b), Interval::constant(1));
}

#[test]
fn test_cmp_gt_definitely_false() {
    let a = Interval::new(0, 5);
    let b = Interval::new(10, 20);
    assert_eq!(a.cmp_gt(&b), Interval::constant(0));
}

#[test]
fn test_cmp_ge_touching() {
    let a = Interval::new(5, 10);
    let b = Interval::new(0, 5);
    // b.hi(5) <= a.lo(5) -> definitely true
    assert_eq!(a.cmp_ge(&b), Interval::constant(1));
}

#[test]
fn test_cmp_ge_definitely_false() {
    let a = Interval::new(0, 3);
    let b = Interval::new(5, 10);
    // a.hi(3) < b.lo(5) -> definitely false
    assert_eq!(a.cmp_ge(&b), Interval::constant(0));
}

#[test]
fn test_cmp_all_bottom_propagates() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(0, 10);
    assert!(bot.cmp_lt(&a).is_empty());
    assert!(bot.cmp_le(&a).is_empty());
    assert!(bot.cmp_gt(&a).is_empty());
    assert!(bot.cmp_ge(&a).is_empty());
    assert!(a.cmp_lt(&bot).is_empty());
    assert!(a.cmp_le(&bot).is_empty());
    assert!(a.cmp_gt(&bot).is_empty());
    assert!(a.cmp_ge(&bot).is_empty());
}
