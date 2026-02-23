//! Pure domain tests for the Interval abstract domain.
//!
//! Tests lattice properties, arithmetic, widening/narrowing, and property-based
//! invariants. No interpreter needed — just the domain types from kirin-test-utils.

use kirin_interpreter::AbstractValue;
use kirin_ir::{HasBottom, HasTop, Lattice};
use kirin_interval::{Bound, Interval, interval_add, interval_sub};

// ============================================================================
// Tests: Lattice properties
// ============================================================================

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

// ============================================================================
// Tests: Widening and Narrowing
// ============================================================================

#[test]
fn test_widen_basic() {
    let a = Interval::new(0, 5);
    let b = Interval::new(0, 10);
    let w = a.widen(&b);
    assert_eq!(w.lo, Bound::Finite(0));
    assert_eq!(w.hi, Bound::PosInf);

    let c = Interval::new(-5, 5);
    let w = a.widen(&c);
    assert_eq!(w.lo, Bound::NegInf);
    assert_eq!(w.hi, Bound::Finite(5));
}

#[test]
fn test_narrow_basic() {
    let wide = Interval {
        lo: Bound::Finite(0),
        hi: Bound::PosInf,
    };
    let refined = Interval::new(0, 100);
    assert_eq!(wide.narrow(&refined), Interval::new(0, 100));

    let wide2 = Interval::top();
    assert_eq!(
        wide2.narrow(&Interval::new(-50, 50)),
        Interval::new(-50, 50)
    );
}

// ============================================================================
// Test: Manual fixpoint iteration (loop analysis)
// ============================================================================

/// Simulates: `x = 0; while (x < 100) { x = x + 1 }; return x`
#[test]
fn test_manual_fixpoint_widening_narrowing() {
    let one = Interval::constant(1);

    // Iteration 1: x = [0,0]
    let mut x = Interval::constant(0);

    // Iteration 2: loop body: x+1 = [1,1], join with entry: [0,1]
    let x_next = interval_add(&x, &one);
    let x_joined = x.join(&x_next);
    assert_eq!(x_joined, Interval::new(0, 1));

    // Widen: lo stable (0), hi grew (0→1) => push hi to +inf
    x = x.widen(&x_joined);
    assert_eq!(x.lo, Bound::Finite(0));
    assert_eq!(x.hi, Bound::PosInf);

    // Iteration 3: [0,+inf) + [1,1] = [1,+inf), join => [0,+inf)
    let x_next2 = interval_add(&x, &one);
    let x_joined2 = x.join(&x_next2);
    let x_widened = x.widen(&x_joined2);
    assert_eq!(x_widened, x); // fixpoint

    // Narrowing: apply constraint x <= 100
    let loop_constraint = Interval::new(0, 100);
    let x_narrowed = x.narrow(&loop_constraint);
    assert_eq!(x_narrowed, Interval::new(0, 100));
}

// ============================================================================
// Property-based tests
// ============================================================================

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
fn test_widen_monotonicity() {
    for x in &representative_intervals() {
        for y in &representative_intervals() {
            let w = x.widen(y);
            assert!(x.is_subseteq(&w), "x={x:?} not subseteq widen(x,y)={w:?}");
            assert!(y.is_subseteq(&w), "y={y:?} not subseteq widen(x,y)={w:?}");
        }
    }
}

#[test]
fn test_narrow_bounds() {
    for x in &representative_intervals() {
        for y in &representative_intervals() {
            let n = x.narrow(y);
            let m = x.meet(y);
            assert!(
                m.is_subseteq(&n),
                "meet not subseteq narrow for x={x:?}, y={y:?}"
            );
            assert!(
                n.is_subseteq(x),
                "narrow not subseteq x for x={x:?}, y={y:?}"
            );
        }
    }
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
