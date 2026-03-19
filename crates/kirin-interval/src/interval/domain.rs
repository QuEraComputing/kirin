use super::Bound;

/// An interval [lo, hi] where lo > hi represents bottom (empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub(crate) lo: Bound,
    pub(crate) hi: Bound,
}

impl Interval {
    /// Returns the lower bound of the interval.
    pub fn lo(&self) -> Bound {
        self.lo
    }

    /// Returns the upper bound of the interval.
    pub fn hi(&self) -> Bound {
        self.hi
    }
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

    pub(crate) fn bottom_interval() -> Self {
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
