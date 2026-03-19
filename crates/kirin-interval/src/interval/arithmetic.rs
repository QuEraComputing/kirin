use kirin_ir::HasTop;

use super::{Bound, Interval};

pub fn interval_add(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.lo.saturating_add(b.lo),
        hi: a.hi.saturating_add(b.hi),
    }
}

pub fn interval_sub(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.lo.saturating_sub(b.hi),
        hi: a.hi.saturating_sub(b.lo),
    }
}

pub fn interval_mul(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }
    let products = [
        a.lo.saturating_mul(b.lo),
        a.lo.saturating_mul(b.hi),
        a.hi.saturating_mul(b.lo),
        a.hi.saturating_mul(b.hi),
    ];
    let lo = products.iter().copied().fold(Bound::PosInf, Bound::min);
    let hi = products.iter().copied().fold(Bound::NegInf, Bound::max);
    Interval { lo, hi }
}

pub fn interval_div(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }

    let zero = Bound::Finite(0);

    // Divisor spans zero → top (division by zero is undefined)
    if b.lo.less_eq(zero) && zero.less_eq(b.hi) {
        return Interval::top();
    }

    // Divisor is strictly positive (b.lo > 0)
    if Bound::Finite(0).less_than(b.lo) {
        let corners = [
            a.lo.saturating_div(b.lo),
            a.lo.saturating_div(b.hi),
            a.hi.saturating_div(b.lo),
            a.hi.saturating_div(b.hi),
        ];
        let lo = corners.iter().copied().fold(Bound::PosInf, Bound::min);
        let hi = corners.iter().copied().fold(Bound::NegInf, Bound::max);
        return Interval { lo, hi };
    }

    // Divisor is strictly negative (b.hi < 0): a / b == (-a) / (-b)
    let neg_a = interval_neg(a);
    let neg_b = interval_neg(b);
    interval_div(&neg_a, &neg_b)
}

pub fn interval_rem(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }

    let zero = Bound::Finite(0);

    // Divisor spans zero → top
    if b.lo.less_eq(zero) && zero.less_eq(b.hi) {
        return Interval::top();
    }

    // M = max(|b.lo|, |b.hi|) - 1
    // Since b does not span zero, both bounds have the same sign.
    let abs_lo = b.lo.negate().max(b.lo);
    let abs_hi = b.hi.negate().max(b.hi);
    let abs_max = abs_lo.max(abs_hi);
    let m = abs_max.saturating_sub(Bound::Finite(1));

    // Bound by sign of a and M
    if zero.less_eq(a.lo) {
        // a is non-negative: result in [0, min(a.hi, M)]
        Interval {
            lo: zero,
            hi: a.hi.min(m),
        }
    } else if a.hi.less_eq(zero) {
        // a is non-positive: result in [max(a.lo, -M), 0]
        Interval {
            lo: a.lo.max(m.negate()),
            hi: zero,
        }
    } else {
        // a spans zero: result in [max(a.lo, -M), min(a.hi, M)]
        Interval {
            lo: a.lo.max(m.negate()),
            hi: a.hi.min(m),
        }
    }
}

pub fn interval_neg(a: &Interval) -> Interval {
    if a.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.hi.negate(),
        hi: a.lo.negate(),
    }
}
