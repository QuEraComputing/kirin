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

// --- i64 boundary tests ---

#[test]
fn test_interval_add_overflow_saturates() {
    // [i64::MAX - 1, i64::MAX] + [1, 2] should saturate via saturating_add
    let a = Interval::new(i64::MAX - 1, i64::MAX);
    let b = Interval::new(1, 2);
    let result = interval_add(&a, &b);
    // saturating_add(i64::MAX - 1, 1) = i64::MAX, saturating_add(i64::MAX, 2) = i64::MAX
    assert_eq!(result.lo, Bound::Finite(i64::MAX));
    assert_eq!(result.hi, Bound::Finite(i64::MAX));
}

#[test]
fn test_interval_sub_underflow_saturates() {
    // [i64::MIN, i64::MIN + 1] - [1, 2] should saturate via saturating_sub
    let a = Interval::new(i64::MIN, i64::MIN + 1);
    let b = Interval::new(1, 2);
    let result = interval_sub(&a, &b);
    // lo = saturating_sub(i64::MIN, 2) = i64::MIN
    // hi = saturating_sub(i64::MIN + 1, 1) = i64::MIN
    assert_eq!(result.lo, Bound::Finite(i64::MIN));
    assert_eq!(result.hi, Bound::Finite(i64::MIN));
}

#[test]
fn test_interval_mul_overflow_saturates() {
    let a = Interval::new(i64::MAX / 2, i64::MAX);
    let b = Interval::new(2, 3);
    let result = interval_mul(&a, &b);
    // All products saturate to i64::MAX except possibly (MAX/2)*2 which is exact
    assert!(!result.is_empty());
    // lo should be (i64::MAX / 2) * 2, which is i64::MAX - 1 (since MAX is odd)
    assert_eq!(result.lo, Bound::Finite((i64::MAX / 2) * 2));
    // hi should be saturating_mul(i64::MAX, 3) = i64::MAX
    assert_eq!(result.hi, Bound::Finite(i64::MAX));
}

#[test]
fn test_bound_negate_i64_min_maps_to_pos_inf() {
    // -i64::MIN overflows i64, so negate maps it to PosInf (sound conservative choice)
    assert_eq!(Bound::Finite(i64::MIN).negate(), Bound::PosInf);
}

#[test]
fn test_interval_add_both_bottom() {
    let bot = Interval::bottom_interval();
    assert!(interval_add(&bot, &bot).is_empty());
}

#[test]
fn test_interval_sub_both_bottom() {
    let bot = Interval::bottom_interval();
    assert!(interval_sub(&bot, &bot).is_empty());
}

#[test]
fn test_interval_mul_both_infinity() {
    let a = Interval::half_bounded_below(1); // [1, +inf)
    let b = Interval::half_bounded_below(1); // [1, +inf)
    let result = interval_mul(&a, &b);
    assert_eq!(result.lo, Bound::Finite(1));
    assert_eq!(result.hi, Bound::PosInf);
}

#[test]
fn test_interval_mul_crossing_zero_with_infinity() {
    let a = Interval::new(-1, 1);
    let b = Interval::half_bounded_below(0); // [0, +inf)
    let result = interval_mul(&a, &b);
    // products: (-1*0)=0, (-1*+inf)=-inf, (1*0)=0, (1*+inf)=+inf
    assert_eq!(result.lo, Bound::NegInf);
    assert_eq!(result.hi, Bound::PosInf);
}

#[test]
fn test_interval_operator_traits() {
    let a = Interval::new(1, 5);
    let b = Interval::new(2, 3);
    assert_eq!(a.clone() + b.clone(), interval_add(&a, &b));
    assert_eq!(a.clone() - b.clone(), interval_sub(&a, &b));
    assert_eq!(a.clone() * b.clone(), interval_mul(&a, &b));
    assert_eq!(-a.clone(), interval_neg(&a));
}

#[test]
fn test_interval_div_positive_by_positive() {
    let a = Interval::new(6, 12);
    let b = Interval::new(2, 3);
    assert_eq!(a / b, Interval::new(2, 6));
}

#[test]
fn test_interval_rem_positive_by_positive() {
    let a = Interval::new(0, 100);
    let b = Interval::new(3, 3);
    assert_eq!(a % b, Interval::new(0, 2));
}

// --- meet/join edge cases ---

#[test]
fn test_meet_disjoint_intervals() {
    let a = Interval::new(0, 5);
    let b = Interval::new(10, 20);
    assert!(a.meet(&b).is_empty());
}

#[test]
fn test_meet_touching_intervals() {
    let a = Interval::new(0, 5);
    let b = Interval::new(5, 10);
    assert_eq!(a.meet(&b), Interval::constant(5));
}

#[test]
fn test_join_disjoint_creates_hull() {
    let a = Interval::new(0, 5);
    let b = Interval::new(10, 20);
    assert_eq!(a.join(&b), Interval::new(0, 20));
}

#[test]
fn test_subseteq_point_in_range() {
    let point = Interval::constant(5);
    let range = Interval::new(0, 10);
    assert!(point.is_subseteq(&range));
}

#[test]
fn test_subseteq_half_bounded() {
    let a = Interval::new(0, 10);
    let b = Interval::half_bounded_below(0);
    assert!(a.is_subseteq(&b));
    assert!(!b.is_subseteq(&a));
}

// --- interval_div tests ---

#[test]
fn test_interval_div_positive_by_positive_corners() {
    // [6, 12] / [2, 3] → corners: 6/2=3, 6/3=2, 12/2=6, 12/3=4 → [2, 6]
    assert_eq!(
        interval_div(&Interval::new(6, 12), &Interval::new(2, 3)),
        Interval::new(2, 6)
    );
}

#[test]
fn test_interval_div_negative_by_positive() {
    // [-12, -6] / [2, 3] → corners: -12/2=-6, -12/3=-4, -6/2=-3, -6/3=-2 → [-6, -2]
    assert_eq!(
        interval_div(&Interval::new(-12, -6), &Interval::new(2, 3)),
        Interval::new(-6, -2)
    );
}

#[test]
fn test_interval_div_mixed_by_positive() {
    // [-6, 6] / [2, 3] → corners: -6/2=-3, -6/3=-2, 6/2=3, 6/3=2 → [-3, 3]
    assert_eq!(
        interval_div(&Interval::new(-6, 6), &Interval::new(2, 3)),
        Interval::new(-3, 3)
    );
}

#[test]
fn test_interval_div_by_zero_spanning() {
    // Divisor spans zero → top
    assert_eq!(
        interval_div(&Interval::new(1, 10), &Interval::new(-1, 1)),
        Interval::top()
    );
    assert_eq!(
        interval_div(&Interval::new(1, 10), &Interval::new(0, 5)),
        Interval::top()
    );
    assert_eq!(
        interval_div(&Interval::new(1, 10), &Interval::new(-5, 0)),
        Interval::top()
    );
}

#[test]
fn test_interval_div_empty_inputs() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(1, 10);
    assert!(interval_div(&bot, &a).is_empty());
    assert!(interval_div(&a, &bot).is_empty());
    assert!(interval_div(&bot, &bot).is_empty());
}

#[test]
fn test_interval_div_point_division() {
    // [5, 5] / [2, 2] → [2, 2] (5/2 = 2 truncated)
    assert_eq!(
        interval_div(&Interval::constant(5), &Interval::constant(2)),
        Interval::constant(2)
    );
}

#[test]
fn test_interval_div_truncation_toward_zero() {
    // [-7, -7] / [2, 2] → [-3, -3] (Rust truncates toward zero: -7/2 = -3)
    assert_eq!(
        interval_div(&Interval::constant(-7), &Interval::constant(2)),
        Interval::constant(-3)
    );
}

#[test]
fn test_interval_div_by_negative() {
    // [6, 12] / [-3, -2] → negate both → [-12, -6] / [2, 3] → [-6, -2]
    // But that's (-a)/(-b), which equals a/b. So [6,12]/[-3,-2] should give [-6, -2]
    // Actually: 6/(-2)=-3, 6/(-3)=-2, 12/(-2)=-6, 12/(-3)=-4 → [-6, -2]
    assert_eq!(
        interval_div(&Interval::new(6, 12), &Interval::new(-3, -2)),
        Interval::new(-6, -2)
    );
}

#[test]
fn test_interval_div_negative_by_negative() {
    // [-12, -6] / [-3, -2] → negate both → [6, 12] / [2, 3] → [2, 6]
    assert_eq!(
        interval_div(&Interval::new(-12, -6), &Interval::new(-3, -2)),
        Interval::new(2, 6)
    );
}

#[test]
fn test_interval_div_with_infinity() {
    // [0, +inf) / [2, 3] → corners: 0/2=0, 0/3=0, +inf/2=+inf, +inf/3=+inf → [0, +inf)
    assert_eq!(
        interval_div(&Interval::half_bounded_below(0), &Interval::new(2, 3)),
        Interval::half_bounded_below(0)
    );

    // (-inf, 0] / [1, 2] → corners: -inf/1=-inf, -inf/2=-inf, 0/1=0, 0/2=0 → (-inf, 0]
    assert_eq!(
        interval_div(&Interval::half_bounded_above(0), &Interval::new(1, 2)),
        Interval::half_bounded_above(0)
    );
}

#[test]
fn test_interval_div_operator_trait() {
    let a = Interval::new(6, 12);
    let b = Interval::new(2, 3);
    assert_eq!(a.clone() / b.clone(), interval_div(&a, &b));
}

// --- interval_rem tests ---

#[test]
fn test_interval_rem_positive_mod() {
    // [0, 100] % [3, 3] → M = 3 - 1 = 2, a non-neg → [0, min(100, 2)] = [0, 2]
    assert_eq!(
        interval_rem(&Interval::new(0, 100), &Interval::constant(3)),
        Interval::new(0, 2)
    );
}

#[test]
fn test_interval_rem_negative_dividend() {
    // [-100, 0] % [3, 3] → M = 2, a non-pos → [max(-100, -2), 0] = [-2, 0]
    assert_eq!(
        interval_rem(&Interval::new(-100, 0), &Interval::constant(3)),
        Interval::new(-2, 0)
    );
}

#[test]
fn test_interval_rem_mixed_dividend() {
    // [-50, 50] % [7, 7] → M = 6, a spans zero → [max(-50, -6), min(50, 6)] = [-6, 6]
    assert_eq!(
        interval_rem(&Interval::new(-50, 50), &Interval::constant(7)),
        Interval::new(-6, 6)
    );
}

#[test]
fn test_interval_rem_small_dividend() {
    // [0, 2] % [10, 10] → M = 9, a non-neg → [0, min(2, 9)] = [0, 2]
    assert_eq!(
        interval_rem(&Interval::new(0, 2), &Interval::constant(10)),
        Interval::new(0, 2)
    );
}

#[test]
fn test_interval_rem_empty_inputs() {
    let bot = Interval::bottom_interval();
    let a = Interval::new(1, 10);
    assert!(interval_rem(&bot, &a).is_empty());
    assert!(interval_rem(&a, &bot).is_empty());
}

#[test]
fn test_interval_rem_zero_spanning_divisor() {
    assert_eq!(
        interval_rem(&Interval::new(1, 10), &Interval::new(-1, 1)),
        Interval::top()
    );
}

#[test]
fn test_interval_rem_negative_divisor() {
    // [0, 100] % [-3, -3] → |b.lo| = 3, |b.hi| = 3, M = 2
    // a non-neg → [0, min(100, 2)] = [0, 2]
    assert_eq!(
        interval_rem(&Interval::new(0, 100), &Interval::constant(-3)),
        Interval::new(0, 2)
    );
}

#[test]
fn test_interval_rem_operator_trait() {
    let a = Interval::new(0, 100);
    let b = Interval::constant(7);
    assert_eq!(a.clone() % b.clone(), interval_rem(&a, &b));
}

// --- Bound::saturating_div tests ---

#[test]
fn test_bound_saturating_div_finite() {
    assert_eq!(
        Bound::Finite(12).saturating_div(Bound::Finite(3)),
        Bound::Finite(4)
    );
    assert_eq!(
        Bound::Finite(7).saturating_div(Bound::Finite(2)),
        Bound::Finite(3)
    );
    assert_eq!(
        Bound::Finite(-7).saturating_div(Bound::Finite(2)),
        Bound::Finite(-3)
    );
    assert_eq!(
        Bound::Finite(-7).saturating_div(Bound::Finite(-2)),
        Bound::Finite(3)
    );
}

#[test]
fn test_bound_saturating_div_inf_by_finite() {
    assert_eq!(
        Bound::PosInf.saturating_div(Bound::Finite(3)),
        Bound::PosInf
    );
    assert_eq!(
        Bound::PosInf.saturating_div(Bound::Finite(-3)),
        Bound::NegInf
    );
    assert_eq!(
        Bound::NegInf.saturating_div(Bound::Finite(3)),
        Bound::NegInf
    );
    assert_eq!(
        Bound::NegInf.saturating_div(Bound::Finite(-3)),
        Bound::PosInf
    );
}

#[test]
fn test_bound_saturating_div_finite_by_inf() {
    assert_eq!(
        Bound::Finite(42).saturating_div(Bound::PosInf),
        Bound::Finite(0)
    );
    assert_eq!(
        Bound::Finite(-42).saturating_div(Bound::NegInf),
        Bound::Finite(0)
    );
}

// --- Soundness: concrete values must be within computed interval ---

#[test]
fn test_interval_div_soundness() {
    let test_cases: Vec<(Interval, Interval)> = vec![
        (Interval::new(1, 10), Interval::new(2, 5)),
        (Interval::new(-10, 10), Interval::new(1, 3)),
        (Interval::new(-20, -5), Interval::new(2, 4)),
        (Interval::new(0, 100), Interval::new(7, 13)),
        (Interval::new(-50, 50), Interval::new(-10, -3)),
    ];

    for (a_iv, b_iv) in &test_cases {
        let result = interval_div(a_iv, b_iv);
        // Check that all concrete a/b values fall within the result
        let a_lo = match a_iv.lo {
            Bound::Finite(v) => v,
            _ => continue,
        };
        let a_hi = match a_iv.hi {
            Bound::Finite(v) => v,
            _ => continue,
        };
        let b_lo = match b_iv.lo {
            Bound::Finite(v) => v,
            _ => continue,
        };
        let b_hi = match b_iv.hi {
            Bound::Finite(v) => v,
            _ => continue,
        };

        for a in a_lo..=a_hi {
            for b in b_lo..=b_hi {
                if b == 0 {
                    continue;
                }
                let concrete = a / b;
                let concrete_iv = Interval::constant(concrete);
                assert!(
                    concrete_iv.is_subseteq(&result),
                    "{a} / {b} = {concrete} not in {result:?} (from {a_iv:?} / {b_iv:?})"
                );
            }
        }
    }
}

#[test]
fn test_interval_rem_soundness() {
    let test_cases: Vec<(Interval, Interval)> = vec![
        (Interval::new(0, 20), Interval::new(3, 7)),
        (Interval::new(-20, 0), Interval::new(3, 7)),
        (Interval::new(-10, 10), Interval::new(1, 5)),
        (Interval::new(0, 5), Interval::new(10, 20)),
        (Interval::new(-30, 30), Interval::new(-7, -3)),
    ];

    for (a_iv, b_iv) in &test_cases {
        let result = interval_rem(a_iv, b_iv);
        let a_lo = match a_iv.lo {
            Bound::Finite(v) => v,
            _ => continue,
        };
        let a_hi = match a_iv.hi {
            Bound::Finite(v) => v,
            _ => continue,
        };
        let b_lo = match b_iv.lo {
            Bound::Finite(v) => v,
            _ => continue,
        };
        let b_hi = match b_iv.hi {
            Bound::Finite(v) => v,
            _ => continue,
        };

        for a in a_lo..=a_hi {
            for b in b_lo..=b_hi {
                if b == 0 {
                    continue;
                }
                let concrete = a % b;
                let concrete_iv = Interval::constant(concrete);
                assert!(
                    concrete_iv.is_subseteq(&result),
                    "{a} % {b} = {concrete} not in {result:?} (from {a_iv:?} % {b_iv:?})"
                );
            }
        }
    }
}
