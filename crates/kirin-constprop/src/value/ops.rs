use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin_arith::{CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;

use super::ConstPropValue;

impl<S, F> ConstPropValue<i64, S, F> {
    fn binary_const(self, rhs: Self, op: impl FnOnce(i64, i64) -> i64) -> Self {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => Self::Const(op(lhs, rhs)),
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            _ => Self::Top,
        }
    }

    fn compare_const(&self, rhs: &Self, compare: impl FnOnce(i64, i64) -> bool) -> Self {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => {
                Self::Const(if compare(*lhs, *rhs) { 1 } else { 0 })
            }
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            _ => Self::Top,
        }
    }
}

impl<S, F> CompareValue for ConstPropValue<i64, S, F> {
    type Bool = Self;

    fn cmp_eq(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, |lhs, rhs| lhs == rhs)
    }

    fn cmp_ne(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, |lhs, rhs| lhs != rhs)
    }

    fn cmp_lt(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, |lhs, rhs| lhs < rhs)
    }

    fn cmp_le(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, |lhs, rhs| lhs <= rhs)
    }

    fn cmp_gt(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, |lhs, rhs| lhs > rhs)
    }

    fn cmp_ge(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, |lhs, rhs| lhs >= rhs)
    }
}

impl<S, F> Add for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, i64::wrapping_add)
    }
}

impl<S, F> Sub for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, i64::wrapping_sub)
    }
}

impl<S, F> Mul for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, i64::wrapping_mul)
    }
}

impl<S, F> Neg for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Const(value) => Self::Const(value.wrapping_neg()),
            value => value,
        }
    }
}

impl<S, F> Not for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Const(value) => Self::Const(!value),
            value => value,
        }
    }
}

impl<S, F> BitAnd for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, |lhs, rhs| lhs & rhs)
    }
}

impl<S, F> BitOr for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, |lhs, rhs| lhs | rhs)
    }
}

impl<S, F> BitXor for ConstPropValue<i64, S, F> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, |lhs, rhs| lhs ^ rhs)
    }
}

impl<S, F> CheckedDiv for ConstPropValue<i64, S, F> {
    fn checked_div(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.checked_div(rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

impl<S, F> CheckedRem for ConstPropValue<i64, S, F> {
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.checked_rem(rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

impl<S, F> CheckedShl for ConstPropValue<i64, S, F> {
    fn checked_shl(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) if (0..64).contains(&rhs) => {
                Some(Self::Const(lhs << rhs))
            }
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

impl<S, F> CheckedShr for ConstPropValue<i64, S, F> {
    fn checked_shr(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) if (0..64).contains(&rhs) => {
                Some(Self::Const(lhs >> rhs))
            }
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

impl<S, F> kirin_scf::ForLoopValue for ConstPropValue<i64, S, F>
where
    S: Clone + PartialEq,
    F: Clone + PartialEq,
{
    fn loop_condition(&self, end: &Self) -> Option<bool> {
        match (self, end) {
            (Self::Const(lhs), Self::Const(rhs)) => Some(*lhs < *rhs),
            _ => None,
        }
    }

    fn loop_step(&self, step: &Self) -> Option<Self> {
        match (self, step) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.checked_add(*rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}
