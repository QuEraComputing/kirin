use super::{Bound, Interval};

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
