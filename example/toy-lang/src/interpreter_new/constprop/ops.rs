use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin_arith::{CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;

use super::ConstProp;

impl CompareValue for ConstProp {
    type Bool = ConstProp;

    fn cmp_eq(&self, other: &Self) -> Self::Bool {
        compare_const(self, other, |lhs, rhs| lhs == rhs)
    }

    fn cmp_ne(&self, other: &Self) -> Self::Bool {
        compare_const(self, other, |lhs, rhs| lhs != rhs)
    }

    fn cmp_lt(&self, other: &Self) -> Self::Bool {
        compare_const(self, other, |lhs, rhs| lhs < rhs)
    }

    fn cmp_le(&self, other: &Self) -> Self::Bool {
        compare_const(self, other, |lhs, rhs| lhs <= rhs)
    }

    fn cmp_gt(&self, other: &Self) -> Self::Bool {
        compare_const(self, other, |lhs, rhs| lhs > rhs)
    }

    fn cmp_ge(&self, other: &Self) -> Self::Bool {
        compare_const(self, other, |lhs, rhs| lhs >= rhs)
    }
}

impl Add for ConstProp {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        binary_const(self, rhs, i64::wrapping_add)
    }
}

impl Sub for ConstProp {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        binary_const(self, rhs, i64::wrapping_sub)
    }
}

impl Mul for ConstProp {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        binary_const(self, rhs, i64::wrapping_mul)
    }
}

impl Neg for ConstProp {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Const(value) => Self::Const(value.wrapping_neg()),
            value => value,
        }
    }
}

impl Not for ConstProp {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Const(value) => Self::Const(!value),
            value => value,
        }
    }
}

impl BitAnd for ConstProp {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        binary_const(self, rhs, |lhs, rhs| lhs & rhs)
    }
}

impl BitOr for ConstProp {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        binary_const(self, rhs, |lhs, rhs| lhs | rhs)
    }
}

impl BitXor for ConstProp {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        binary_const(self, rhs, |lhs, rhs| lhs ^ rhs)
    }
}

impl CheckedDiv for ConstProp {
    fn checked_div(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.checked_div(rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

impl CheckedRem for ConstProp {
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.checked_rem(rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

impl CheckedShl for ConstProp {
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

impl CheckedShr for ConstProp {
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

fn compare_const(
    lhs: &ConstProp,
    rhs: &ConstProp,
    compare: impl FnOnce(i64, i64) -> bool,
) -> ConstProp {
    match (lhs, rhs) {
        (ConstProp::Const(lhs), ConstProp::Const(rhs)) => {
            ConstProp::Const(if compare(*lhs, *rhs) { 1 } else { 0 })
        }
        (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
        _ => ConstProp::Top,
    }
}

fn binary_const(lhs: ConstProp, rhs: ConstProp, op: impl FnOnce(i64, i64) -> i64) -> ConstProp {
    match (lhs, rhs) {
        (ConstProp::Const(lhs), ConstProp::Const(rhs)) => ConstProp::Const(op(lhs, rhs)),
        (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
        _ => ConstProp::Top,
    }
}
