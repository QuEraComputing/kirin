use super::Interval;
use crate::BoolInterval;

impl kirin_cmp::CompareValue for Interval {
    type Bool = BoolInterval;

    fn cmp_eq(&self, other: &Self) -> Self::Bool {
        if self.is_empty() || other.is_empty() {
            return BoolInterval::Bottom;
        }
        // Both are single points and equal -> definitely true
        if self == other && self.lo == self.hi {
            return BoolInterval::True;
        }
        // Ranges are disjoint -> definitely false
        if self.hi.less_than(other.lo) || other.hi.less_than(self.lo) {
            return BoolInterval::False;
        }
        BoolInterval::Unknown
    }

    fn cmp_ne(&self, other: &Self) -> Self::Bool {
        if self.is_empty() || other.is_empty() {
            return BoolInterval::Bottom;
        }
        if self == other && self.lo == self.hi {
            return BoolInterval::False;
        }
        if self.hi.less_than(other.lo) || other.hi.less_than(self.lo) {
            return BoolInterval::True;
        }
        BoolInterval::Unknown
    }

    fn cmp_lt(&self, other: &Self) -> Self::Bool {
        if self.is_empty() || other.is_empty() {
            return BoolInterval::Bottom;
        }
        // self.hi < other.lo -> definitely true
        if self.hi.less_than(other.lo) {
            return BoolInterval::True;
        }
        // other.hi <= self.lo -> definitely false
        if other.hi.less_eq(self.lo) {
            return BoolInterval::False;
        }
        BoolInterval::Unknown
    }

    fn cmp_le(&self, other: &Self) -> Self::Bool {
        if self.is_empty() || other.is_empty() {
            return BoolInterval::Bottom;
        }
        if self.hi.less_eq(other.lo) {
            return BoolInterval::True;
        }
        if other.hi.less_than(self.lo) {
            return BoolInterval::False;
        }
        BoolInterval::Unknown
    }

    fn cmp_gt(&self, other: &Self) -> Self::Bool {
        if self.is_empty() || other.is_empty() {
            return BoolInterval::Bottom;
        }
        if other.hi.less_than(self.lo) {
            return BoolInterval::True;
        }
        if self.hi.less_eq(other.lo) {
            return BoolInterval::False;
        }
        BoolInterval::Unknown
    }

    fn cmp_ge(&self, other: &Self) -> Self::Bool {
        if self.is_empty() || other.is_empty() {
            return BoolInterval::Bottom;
        }
        if other.hi.less_eq(self.lo) {
            return BoolInterval::True;
        }
        if self.hi.less_than(other.lo) {
            return BoolInterval::False;
        }
        BoolInterval::Unknown
    }
}
