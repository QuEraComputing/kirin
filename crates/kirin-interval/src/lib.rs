use kirin_ir::{HasBottom, HasTop, Lattice};

// ============================================================================
// Interval Domain
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    NegInf,
    Finite(i64),
    PosInf,
}

impl Bound {
    pub fn min(self, other: Self) -> Self {
        match (self, other) {
            (Bound::NegInf, _) | (_, Bound::NegInf) => Bound::NegInf,
            (Bound::PosInf, b) | (b, Bound::PosInf) => b,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.min(b)),
        }
    }

    pub fn max(self, other: Self) -> Self {
        match (self, other) {
            (Bound::PosInf, _) | (_, Bound::PosInf) => Bound::PosInf,
            (Bound::NegInf, b) | (b, Bound::NegInf) => b,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.max(b)),
        }
    }

    pub fn less_than(self, other: Self) -> bool {
        match (self, other) {
            (Bound::NegInf, Bound::NegInf) => false,
            (Bound::NegInf, _) => true,
            (_, Bound::NegInf) => false,
            (Bound::PosInf, _) => false,
            (_, Bound::PosInf) => true,
            (Bound::Finite(a), Bound::Finite(b)) => a < b,
        }
    }

    pub fn less_eq(self, other: Self) -> bool {
        self == other || self.less_than(other)
    }

    pub fn saturating_add(self, other: Self) -> Self {
        match (self, other) {
            (Bound::NegInf, Bound::PosInf) | (Bound::PosInf, Bound::NegInf) => Bound::NegInf,
            (Bound::NegInf, _) | (_, Bound::NegInf) => Bound::NegInf,
            (Bound::PosInf, _) | (_, Bound::PosInf) => Bound::PosInf,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.saturating_add(b)),
        }
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        match (self, other) {
            (Bound::NegInf, Bound::NegInf) | (Bound::PosInf, Bound::PosInf) => Bound::NegInf,
            (Bound::NegInf, _) | (_, Bound::PosInf) => Bound::NegInf,
            (Bound::PosInf, _) | (_, Bound::NegInf) => Bound::PosInf,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.saturating_sub(b)),
        }
    }

    pub fn saturating_mul(self, other: Self) -> Self {
        match (self, other) {
            (Bound::Finite(0), _) | (_, Bound::Finite(0)) => Bound::Finite(0),
            (Bound::NegInf, Bound::NegInf) | (Bound::PosInf, Bound::PosInf) => Bound::PosInf,
            (Bound::NegInf, Bound::PosInf) | (Bound::PosInf, Bound::NegInf) => Bound::NegInf,
            (Bound::NegInf, Bound::Finite(b)) | (Bound::Finite(b), Bound::NegInf) => {
                if b > 0 {
                    Bound::NegInf
                } else {
                    Bound::PosInf
                }
            }
            (Bound::PosInf, Bound::Finite(b)) | (Bound::Finite(b), Bound::PosInf) => {
                if b > 0 {
                    Bound::PosInf
                } else {
                    Bound::NegInf
                }
            }
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.saturating_mul(b)),
        }
    }

    pub fn negate(self) -> Self {
        match self {
            Bound::NegInf => Bound::PosInf,
            Bound::PosInf => Bound::NegInf,
            Bound::Finite(v) => Bound::Finite(-v),
        }
    }
}

/// An interval [lo, hi] where lo > hi represents bottom (empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub lo: Bound,
    pub hi: Bound,
}

impl Interval {
    pub fn new(lo: i64, hi: i64) -> Self {
        if lo > hi {
            Self::bottom_interval()
        } else {
            Interval {
                lo: Bound::Finite(lo),
                hi: Bound::Finite(hi),
            }
        }
    }

    pub fn constant(v: i64) -> Self {
        Interval::new(v, v)
    }

    pub fn bottom_interval() -> Self {
        Interval {
            lo: Bound::PosInf,
            hi: Bound::NegInf,
        }
    }

    pub fn half_bounded_above(hi: i64) -> Self {
        Interval {
            lo: Bound::NegInf,
            hi: Bound::Finite(hi),
        }
    }

    pub fn half_bounded_below(lo: i64) -> Self {
        Interval {
            lo: Bound::Finite(lo),
            hi: Bound::PosInf,
        }
    }

    pub fn is_empty(&self) -> bool {
        match (self.lo, self.hi) {
            (Bound::PosInf, _) => true,
            (_, Bound::NegInf) => true,
            (Bound::Finite(lo), Bound::Finite(hi)) => lo > hi,
            (Bound::NegInf, _) => false,
            (_, Bound::PosInf) => false,
        }
    }
}

// ============================================================================
// Interval Arithmetic
// ============================================================================

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

// ============================================================================
// Lattice + AbstractValue impls
// ============================================================================

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

#[cfg(feature = "interpreter")]
impl kirin_interpreter::BranchCondition for Interval {
    fn is_truthy(&self) -> Option<bool> {
        if self.is_empty() {
            return None;
        }
        let all_negative = match self.hi {
            Bound::NegInf => true,
            Bound::Finite(h) => h < 0,
            Bound::PosInf => false,
        };
        let all_positive = match self.lo {
            Bound::PosInf => true,
            Bound::Finite(l) => l > 0,
            Bound::NegInf => false,
        };
        if all_negative || all_positive {
            return Some(true);
        }
        if *self == Interval::constant(0) {
            return Some(false);
        }
        None
    }
}

#[cfg(feature = "arith")]
impl From<kirin_arith::ArithValue> for Interval {
    fn from(v: kirin_arith::ArithValue) -> Self {
        use kirin_arith::ArithValue;
        match v {
            ArithValue::I64(x) => Interval::constant(x),
            ArithValue::I32(x) => Interval::constant(x as i64),
            ArithValue::I16(x) => Interval::constant(x as i64),
            ArithValue::I8(x) => Interval::constant(x as i64),
            _ => Interval::top(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::lattice::assert_finite_lattice_laws;

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
}

#[cfg(feature = "interpreter")]
impl kirin_interpreter::AbstractValue for Interval {
    fn widen(&self, next: &Self) -> Self {
        if self.is_empty() {
            return next.clone();
        }
        if next.is_empty() {
            return self.clone();
        }
        let lo = if next.lo.less_than(self.lo) {
            Bound::NegInf
        } else {
            self.lo
        };
        let hi = if self.hi.less_than(next.hi) {
            Bound::PosInf
        } else {
            self.hi
        };
        Interval { lo, hi }
    }

    fn narrow(&self, next: &Self) -> Self {
        if self.is_empty() || next.is_empty() {
            return self.clone();
        }
        let lo = match self.lo {
            Bound::NegInf => next.lo,
            other => other,
        };
        let hi = match self.hi {
            Bound::PosInf => next.hi,
            other => other,
        };
        Interval { lo, hi }
    }
}
