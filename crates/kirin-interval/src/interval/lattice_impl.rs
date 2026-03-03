use kirin_ir::{HasBottom, HasTop, Lattice};

use super::{Bound, Interval, interval_add, interval_mul, interval_neg, interval_sub};

impl Lattice for Interval {
    fn join(&self, other: &Self) -> Self {
        if self.is_empty() {
            return other.clone();
        }
        if other.is_empty() {
            return self.clone();
        }
        Interval {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        if lo.less_eq(hi) {
            Interval { lo, hi }
        } else {
            Interval::bottom_interval()
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        if self.is_empty() {
            return true;
        }
        if other.is_empty() {
            return false;
        }
        other.lo.less_eq(self.lo) && self.hi.less_eq(other.hi)
    }
}

impl HasBottom for Interval {
    fn bottom() -> Self {
        Interval::bottom_interval()
    }
}

impl HasTop for Interval {
    fn top() -> Self {
        Interval {
            lo: Bound::NegInf,
            hi: Bound::PosInf,
        }
    }
}

impl std::ops::Add for Interval {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        interval_add(&self, &rhs)
    }
}

impl std::ops::Sub for Interval {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        interval_sub(&self, &rhs)
    }
}

impl std::ops::Mul for Interval {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        interval_mul(&self, &rhs)
    }
}

impl std::ops::Div for Interval {
    type Output = Self;

    fn div(self, _rhs: Self) -> Self {
        Interval::top()
    }
}

impl std::ops::Rem for Interval {
    type Output = Self;

    fn rem(self, _rhs: Self) -> Self {
        Interval::top()
    }
}

impl std::ops::Neg for Interval {
    type Output = Self;

    fn neg(self) -> Self {
        interval_neg(&self)
    }
}
