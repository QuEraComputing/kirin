use super::Interval;

impl kirin_cmp::CompareValue for Interval {
    fn cmp_eq(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        // Both are single points and equal -> definitely 1
        if self == other && self.lo == self.hi {
            return Interval::constant(1);
        }
        // Ranges are disjoint -> definitely 0
        if self.hi.less_than(other.lo) || other.hi.less_than(self.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_ne(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if self == other && self.lo == self.hi {
            return Interval::constant(0);
        }
        if self.hi.less_than(other.lo) || other.hi.less_than(self.lo) {
            return Interval::constant(1);
        }
        Interval::new(0, 1)
    }

    fn cmp_lt(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        // self.hi < other.lo -> definitely true
        if self.hi.less_than(other.lo) {
            return Interval::constant(1);
        }
        // other.hi <= self.lo -> definitely false
        if other.hi.less_eq(self.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_le(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if self.hi.less_eq(other.lo) {
            return Interval::constant(1);
        }
        if other.hi.less_than(self.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_gt(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if other.hi.less_than(self.lo) {
            return Interval::constant(1);
        }
        if self.hi.less_eq(other.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_ge(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if other.hi.less_eq(self.lo) {
            return Interval::constant(1);
        }
        if self.hi.less_than(other.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }
}
