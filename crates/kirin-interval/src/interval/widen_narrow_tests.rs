use kirin_interpreter::AbstractValue;
use kirin_ir::Lattice;

use super::*;

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

    // Widen: lo stable (0), hi grew (0->1) => push hi to +inf
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
