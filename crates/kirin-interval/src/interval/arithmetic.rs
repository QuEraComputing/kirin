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

pub fn interval_neg(a: &Interval) -> Interval {
    if a.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.hi.negate(),
        hi: a.lo.negate(),
    }
}
