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

// --- Bound arithmetic tests ---

#[test]
fn test_bound_min() {
    assert_eq!(Bound::NegInf.min(Bound::PosInf), Bound::NegInf);
    assert_eq!(Bound::PosInf.min(Bound::NegInf), Bound::NegInf);
    assert_eq!(Bound::NegInf.min(Bound::Finite(0)), Bound::NegInf);
    assert_eq!(Bound::PosInf.min(Bound::Finite(0)), Bound::Finite(0));
    assert_eq!(Bound::Finite(3).min(Bound::Finite(7)), Bound::Finite(3));
    assert_eq!(Bound::Finite(7).min(Bound::Finite(3)), Bound::Finite(3));
}

#[test]
fn test_bound_max() {
    assert_eq!(Bound::NegInf.max(Bound::PosInf), Bound::PosInf);
    assert_eq!(Bound::PosInf.max(Bound::NegInf), Bound::PosInf);
    assert_eq!(Bound::NegInf.max(Bound::Finite(0)), Bound::Finite(0));
    assert_eq!(Bound::PosInf.max(Bound::Finite(0)), Bound::PosInf);
    assert_eq!(Bound::Finite(3).max(Bound::Finite(7)), Bound::Finite(7));
}

#[test]
fn test_bound_negate() {
    assert_eq!(Bound::NegInf.negate(), Bound::PosInf);
    assert_eq!(Bound::PosInf.negate(), Bound::NegInf);
    assert_eq!(Bound::Finite(5).negate(), Bound::Finite(-5));
    assert_eq!(Bound::Finite(0).negate(), Bound::Finite(0));
    assert_eq!(Bound::Finite(-3).negate(), Bound::Finite(3));
}

#[test]
fn test_bound_saturating_add() {
    // Finite + Finite
    assert_eq!(
        Bound::Finite(3).saturating_add(Bound::Finite(4)),
        Bound::Finite(7)
    );
    // Inf propagation
    assert_eq!(
        Bound::NegInf.saturating_add(Bound::Finite(10)),
        Bound::NegInf
    );
    assert_eq!(
        Bound::PosInf.saturating_add(Bound::Finite(10)),
        Bound::PosInf
    );
    assert_eq!(
        Bound::Finite(10).saturating_add(Bound::NegInf),
        Bound::NegInf
    );
    assert_eq!(
        Bound::Finite(10).saturating_add(Bound::PosInf),
        Bound::PosInf
    );
    // DESIGN NOTE: NegInf + PosInf = NegInf (asymmetric choice — NegInf dominates)
    assert_eq!(Bound::NegInf.saturating_add(Bound::PosInf), Bound::NegInf);
    assert_eq!(Bound::PosInf.saturating_add(Bound::NegInf), Bound::NegInf);
}

#[test]
fn test_bound_saturating_sub() {
    assert_eq!(
        Bound::Finite(10).saturating_sub(Bound::Finite(3)),
        Bound::Finite(7)
    );
    // DESIGN NOTE: NegInf - NegInf = NegInf, PosInf - PosInf = NegInf (same-sign = NegInf)
    assert_eq!(Bound::NegInf.saturating_sub(Bound::NegInf), Bound::NegInf);
    assert_eq!(Bound::PosInf.saturating_sub(Bound::PosInf), Bound::NegInf);
    // Opposite signs
    assert_eq!(Bound::PosInf.saturating_sub(Bound::NegInf), Bound::PosInf);
    assert_eq!(Bound::NegInf.saturating_sub(Bound::PosInf), Bound::NegInf);
    // Inf - Finite
    assert_eq!(
        Bound::NegInf.saturating_sub(Bound::Finite(5)),
        Bound::NegInf
    );
    assert_eq!(
        Bound::PosInf.saturating_sub(Bound::Finite(5)),
        Bound::PosInf
    );
    // Finite - Inf
    assert_eq!(
        Bound::Finite(5).saturating_sub(Bound::PosInf),
        Bound::NegInf
    );
    assert_eq!(
        Bound::Finite(5).saturating_sub(Bound::NegInf),
        Bound::PosInf
    );
}

#[test]
fn test_bound_saturating_mul() {
    // Finite * Finite
    assert_eq!(
        Bound::Finite(3).saturating_mul(Bound::Finite(4)),
        Bound::Finite(12)
    );
    assert_eq!(
        Bound::Finite(-3).saturating_mul(Bound::Finite(4)),
        Bound::Finite(-12)
    );
    assert_eq!(
        Bound::Finite(-3).saturating_mul(Bound::Finite(-4)),
        Bound::Finite(12)
    );
    // Zero absorbs infinity
    assert_eq!(
        Bound::Finite(0).saturating_mul(Bound::NegInf),
        Bound::Finite(0)
    );
    assert_eq!(
        Bound::Finite(0).saturating_mul(Bound::PosInf),
        Bound::Finite(0)
    );
    assert_eq!(
        Bound::NegInf.saturating_mul(Bound::Finite(0)),
        Bound::Finite(0)
    );
    assert_eq!(
        Bound::PosInf.saturating_mul(Bound::Finite(0)),
        Bound::Finite(0)
    );
    // Inf * Inf (same sign = PosInf, opposite = NegInf)
    assert_eq!(Bound::NegInf.saturating_mul(Bound::NegInf), Bound::PosInf);
    assert_eq!(Bound::PosInf.saturating_mul(Bound::PosInf), Bound::PosInf);
    assert_eq!(Bound::NegInf.saturating_mul(Bound::PosInf), Bound::NegInf);
    assert_eq!(Bound::PosInf.saturating_mul(Bound::NegInf), Bound::NegInf);
    // Inf * positive finite
    assert_eq!(
        Bound::PosInf.saturating_mul(Bound::Finite(5)),
        Bound::PosInf
    );
    assert_eq!(
        Bound::NegInf.saturating_mul(Bound::Finite(5)),
        Bound::NegInf
    );
    // Inf * negative finite
    assert_eq!(
        Bound::PosInf.saturating_mul(Bound::Finite(-5)),
        Bound::NegInf
    );
    assert_eq!(
        Bound::NegInf.saturating_mul(Bound::Finite(-5)),
        Bound::PosInf
    );
}

#[test]
fn test_bound_less_than() {
    assert!(Bound::NegInf.less_than(Bound::PosInf));
    assert!(Bound::NegInf.less_than(Bound::Finite(0)));
    assert!(Bound::Finite(0).less_than(Bound::PosInf));
    assert!(Bound::Finite(1).less_than(Bound::Finite(2)));
    assert!(!Bound::NegInf.less_than(Bound::NegInf));
    assert!(!Bound::PosInf.less_than(Bound::PosInf));
    assert!(!Bound::PosInf.less_than(Bound::NegInf));
    assert!(!Bound::Finite(2).less_than(Bound::Finite(1)));
    assert!(!Bound::Finite(1).less_than(Bound::Finite(1)));
}

#[test]
fn test_bound_less_eq() {
    assert!(Bound::NegInf.less_eq(Bound::NegInf));
    assert!(Bound::PosInf.less_eq(Bound::PosInf));
    assert!(Bound::Finite(5).less_eq(Bound::Finite(5)));
    assert!(Bound::Finite(3).less_eq(Bound::Finite(5)));
    assert!(!Bound::Finite(5).less_eq(Bound::Finite(3)));
}

// --- interval_mul tests ---

#[test]
fn test_interval_mul_positive() {
    let a = Interval::new(2, 3);
    let b = Interval::new(4, 5);
    assert_eq!(interval_mul(&a, &b), Interval::new(8, 15));
}

#[test]
fn test_interval_mul_negative_times_negative() {
    let a = Interval::new(-3, -2);
    let b = Interval::new(-5, -4);
    assert_eq!(interval_mul(&a, &b), Interval::new(8, 15));
}

#[test]
fn test_interval_mul_mixed_sign() {
    let a = Interval::new(-2, 3);
    let b = Interval::new(-1, 4);
    // products: (-2*-1)=2, (-2*4)=-8, (3*-1)=-3, (3*4)=12
    assert_eq!(interval_mul(&a, &b), Interval::new(-8, 12));
}

#[test]
fn test_interval_mul_bottom_propagates() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(1, 5);
    assert!(interval_mul(&bot, &a).is_empty());
    assert!(interval_mul(&a, &bot).is_empty());
}

#[test]
fn test_interval_mul_by_zero() {
    let zero = Interval::constant(0);
    let a = Interval::new(-10, 10);
    assert_eq!(interval_mul(&zero, &a), Interval::constant(0));
}

// --- interval_neg tests ---

#[test]
fn test_interval_neg_finite() {
    let a = Interval::new(1, 5);
    assert_eq!(interval_neg(&a), Interval::new(-5, -1));
}

#[test]
fn test_interval_neg_mixed() {
    let a = Interval::new(-3, 7);
    assert_eq!(interval_neg(&a), Interval::new(-7, 3));
}

#[test]
fn test_interval_neg_bottom() {
    assert!(interval_neg(&Interval::bottom_interval()).is_empty());
}

#[test]
fn test_interval_neg_top() {
    let top = Interval::top();
    let neg = interval_neg(&top);
    assert_eq!(neg.lo, Bound::NegInf);
    assert_eq!(neg.hi, Bound::PosInf);
}

#[test]
fn test_interval_neg_half_bounded() {
    let a = Interval::half_bounded_below(0); // [0, +inf)
    let neg = interval_neg(&a);
    assert_eq!(neg.lo, Bound::NegInf);
    assert_eq!(neg.hi, Bound::Finite(0));
}

// --- interval_sub with infinity ---

#[test]
fn test_interval_sub_with_infinity() {
    let a = Interval::half_bounded_below(0); // [0, +inf)
    let b = Interval::constant(1);
    let result = interval_sub(&a, &b);
    assert_eq!(result.lo, Bound::Finite(-1));
    assert_eq!(result.hi, Bound::PosInf);
}

// --- meet associativity and commutativity ---

#[test]
fn test_meet_commutativity() {
    for a in &representative_intervals() {
        for b in &representative_intervals() {
            assert_eq!(
                a.meet(b),
                b.meet(a),
                "meet not commutative for {a:?}, {b:?}"
            );
        }
    }
}

#[test]
fn test_meet_associativity() {
    for a in &representative_intervals() {
        for b in &representative_intervals() {
            for c in &representative_intervals() {
                assert_eq!(
                    a.meet(b).meet(c),
                    a.meet(&b.meet(c)),
                    "meet not associative for {a:?}, {b:?}, {c:?}"
                );
            }
        }
    }
}

#[test]
fn test_top_is_identity_for_meet() {
    let top = Interval::top();
    for a in &representative_intervals() {
        assert_eq!(a.meet(&top), *a, "meet(a, top) != a for {a:?}");
        assert_eq!(top.meet(a), *a, "meet(top, a) != a for {a:?}");
    }
}

// --- is_empty edge cases ---

#[test]
fn test_is_empty_edge_cases() {
    assert!(Interval::bottom_interval().is_empty());
    assert!(!Interval::constant(0).is_empty());
    assert!(!Interval::top().is_empty());
    // lo=PosInf always empty
    assert!(
        Interval {
            lo: Bound::PosInf,
            hi: Bound::PosInf
        }
        .is_empty()
    );
    // hi=NegInf always empty
    assert!(
        Interval {
            lo: Bound::NegInf,
            hi: Bound::NegInf
        }
        .is_empty()
    );
    // Finite reversed
    assert!(Interval::new(10, 5).is_empty());
    // Finite equal (point)
    assert!(!Interval::new(5, 5).is_empty());
}

// --- Interval::new normalizes reversed bounds ---

#[test]
fn test_new_reversed_becomes_bottom() {
    let reversed = Interval::new(10, 5);
    assert_eq!(reversed, Interval::bottom_interval());
}
