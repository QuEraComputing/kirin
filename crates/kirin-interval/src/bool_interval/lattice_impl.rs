use kirin_ir::{HasBottom, HasTop, Lattice};

use super::BoolInterval;

impl Lattice for BoolInterval {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            // Bottom is the identity for join
            (BoolInterval::Bottom, x) | (x, BoolInterval::Bottom) => *x,
            // Unknown absorbs everything
            (BoolInterval::Unknown, _) | (_, BoolInterval::Unknown) => BoolInterval::Unknown,
            // Same values
            (BoolInterval::True, BoolInterval::True) => BoolInterval::True,
            (BoolInterval::False, BoolInterval::False) => BoolInterval::False,
            // Different concrete values -> Unknown
            (BoolInterval::True, BoolInterval::False)
            | (BoolInterval::False, BoolInterval::True) => BoolInterval::Unknown,
        }
    }

    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            // Unknown is the identity for meet
            (BoolInterval::Unknown, x) | (x, BoolInterval::Unknown) => *x,
            // Bottom absorbs everything
            (BoolInterval::Bottom, _) | (_, BoolInterval::Bottom) => BoolInterval::Bottom,
            // Same values
            (BoolInterval::True, BoolInterval::True) => BoolInterval::True,
            (BoolInterval::False, BoolInterval::False) => BoolInterval::False,
            // Different concrete values -> Bottom
            (BoolInterval::True, BoolInterval::False)
            | (BoolInterval::False, BoolInterval::True) => BoolInterval::Bottom,
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        match (self, other) {
            // Bottom is subset of everything
            (BoolInterval::Bottom, _) => true,
            // Everything is subset of Unknown (top)
            (_, BoolInterval::Unknown) => true,
            // Same values
            (a, b) => a == b,
        }
    }
}

impl HasBottom for BoolInterval {
    fn bottom() -> Self {
        BoolInterval::Bottom
    }
}

impl HasTop for BoolInterval {
    fn top() -> Self {
        BoolInterval::Unknown
    }
}
