use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin_ir::HasTop;

use crate::interval::Interval;

impl BitAnd for Interval {
    type Output = Self;
    fn bitand(self, _rhs: Self) -> Self {
        Self::top()
    }
}

impl BitOr for Interval {
    type Output = Self;
    fn bitor(self, _rhs: Self) -> Self {
        Self::top()
    }
}

impl BitXor for Interval {
    type Output = Self;
    fn bitxor(self, _rhs: Self) -> Self {
        Self::top()
    }
}

impl Not for Interval {
    type Output = Self;
    fn not(self) -> Self {
        Self::top()
    }
}

impl kirin_bitwise::CheckedShl for Interval {
    fn checked_shl(self, _rhs: Self) -> Option<Self> {
        Some(Self::top())
    }
}

impl kirin_bitwise::CheckedShr for Interval {
    fn checked_shr(self, _rhs: Self) -> Option<Self> {
        Some(Self::top())
    }
}
