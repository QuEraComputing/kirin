use kirin_ir::{HasBottom, HasTop, Lattice};
use kirin_test_utils::lattice::assert_finite_lattice_laws;

use super::*;

#[test]
fn interval_lattice_laws() {
    let elements = vec![
        Interval::bottom(),
        Interval::constant(0),
        Interval::constant(42),
        Interval::new(0, 10),
        Interval::new(-5, 5),
        Interval::new(3, 7),
        Interval::new(-100, 100),
        Interval::top(),
    ];
    assert_finite_lattice_laws(&elements);
}

#[test]
fn test_interval_lattice_basic() {
    let a = Interval::new(0, 10);
    let b = Interval::new(5, 20);

    assert_eq!(a.join(&b), Interval::new(0, 20));
    assert_eq!(a.meet(&b), Interval::new(5, 10));
    assert!(Interval::new(2, 8).is_subseteq(&a));
    assert!(!b.is_subseteq(&a));

    let bot = <Interval as HasBottom>::bottom();
    assert!(bot.is_empty());
    assert!(bot.is_subseteq(&a));
    assert_eq!(bot.join(&a), a);
    assert_eq!(a.meet(&bot), bot);

    let top = Interval::top();
    assert!(a.is_subseteq(&top));
    assert_eq!(a.join(&top), top);
    assert_eq!(a.meet(&top), a);
}

#[test]
fn test_interval_arithmetic() {
    let a = Interval::new(1, 5);
    let b = Interval::new(10, 20);

    assert_eq!(interval_add(&a, &b), Interval::new(11, 25));
    assert_eq!(interval_sub(&b, &a), Interval::new(5, 19));
    assert_eq!(Interval::constant(42), Interval::new(42, 42));
}

#[test]
fn test_interval_arithmetic_with_infinity() {
    let a = Interval::half_bounded_below(0);
    let b = Interval::constant(1);
    let sum = interval_add(&a, &b);
    assert_eq!(sum.lo, Bound::Finite(1));
    assert_eq!(sum.hi, Bound::PosInf);
}

fn representative_intervals() -> Vec<Interval> {
    vec![
        Interval::bottom_interval(),
        Interval::top(),
        Interval::constant(0),
        Interval::constant(42),
        Interval::constant(-10),
        Interval::new(0, 100),
        Interval::new(-50, 50),
        Interval::new(1, 1),
        Interval::half_bounded_below(0),
        Interval::half_bounded_above(100),
        Interval::new(-1000, 1000),
    ]
}

#[test]
fn test_join_associativity() {
    for a in &representative_intervals() {
        for b in &representative_intervals() {
            for c in &representative_intervals() {
                assert_eq!(a.join(b).join(c), a.join(&b.join(c)));
            }
        }
    }
}

#[test]
fn test_join_commutativity() {
    for a in &representative_intervals() {
        for b in &representative_intervals() {
            assert_eq!(a.join(b), b.join(a));
        }
    }
}

#[test]
fn test_bottom_is_identity_for_join() {
    let bot = <Interval as HasBottom>::bottom();
    for a in &representative_intervals() {
        assert_eq!(a.join(&bot), *a);
        assert_eq!(bot.join(a), *a);
    }
}
