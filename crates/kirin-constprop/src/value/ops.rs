//! Scalar transfer functions, lifted from the constant type `C`.
//!
//! Known constants use `C`'s operations. Addition, subtraction, multiplication,
//! and negation use checked operations and return `Top` on overflow.

use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin_arith::{CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use num_traits::{CheckedAdd, CheckedMul, CheckedNeg, CheckedSub};

use super::ConstPropValue;

impl<C, S, F> ConstPropValue<C, S, F> {
    fn unary_const(self, op: impl FnOnce(C) -> C) -> Self {
        match self {
            Self::Const(value) => Self::Const(op(value)),
            value => value,
        }
    }

    fn binary_const(self, rhs: Self, op: impl FnOnce(C, C) -> C) -> Self {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => Self::Const(op(lhs, rhs)),
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            _ => Self::Top,
        }
    }

    /// `None` when the concrete operation fails on two known constants.
    fn checked_const(self, rhs: Self, op: impl FnOnce(C, C) -> Option<C>) -> Option<Self> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => op(lhs, rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }

    fn compare_const<B>(
        &self,
        rhs: &Self,
        compare: impl FnOnce(&C, &C) -> B,
    ) -> ConstPropValue<B, S, F> {
        match (self, rhs) {
            (Self::Const(lhs), Self::Const(rhs)) => ConstPropValue::Const(compare(lhs, rhs)),
            (Self::Bottom, _) | (_, Self::Bottom) => ConstPropValue::Bottom,
            _ => ConstPropValue::Top,
        }
    }
}

impl<C: CompareValue, S, F> CompareValue for ConstPropValue<C, S, F> {
    type Bool = ConstPropValue<C::Bool, S, F>;

    fn cmp_eq(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, C::cmp_eq)
    }

    fn cmp_ne(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, C::cmp_ne)
    }

    fn cmp_lt(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, C::cmp_lt)
    }

    fn cmp_le(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, C::cmp_le)
    }

    fn cmp_gt(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, C::cmp_gt)
    }

    fn cmp_ge(&self, other: &Self) -> Self::Bool {
        self.compare_const(other, C::cmp_ge)
    }
}

impl<C: CheckedAdd, S, F> Add for ConstPropValue<C, S, F> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.checked_const(rhs, |lhs, rhs| lhs.checked_add(&rhs))
            .unwrap_or(Self::Top)
    }
}

impl<C: CheckedSub, S, F> Sub for ConstPropValue<C, S, F> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_const(rhs, |lhs, rhs| lhs.checked_sub(&rhs))
            .unwrap_or(Self::Top)
    }
}

impl<C: CheckedMul, S, F> Mul for ConstPropValue<C, S, F> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_const(rhs, |lhs, rhs| lhs.checked_mul(&rhs))
            .unwrap_or(Self::Top)
    }
}

impl<C: CheckedNeg, S, F> Neg for ConstPropValue<C, S, F> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Const(value) => value.checked_neg().map_or(Self::Top, Self::Const),
            value => value,
        }
    }
}

impl<C: Not<Output = C>, S, F> Not for ConstPropValue<C, S, F> {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.unary_const(C::not)
    }
}

impl<C: BitAnd<Output = C>, S, F> BitAnd for ConstPropValue<C, S, F> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, C::bitand)
    }
}

impl<C: BitOr<Output = C>, S, F> BitOr for ConstPropValue<C, S, F> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, C::bitor)
    }
}

impl<C: BitXor<Output = C>, S, F> BitXor for ConstPropValue<C, S, F> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        self.binary_const(rhs, C::bitxor)
    }
}

impl<C: CheckedDiv, S, F> CheckedDiv for ConstPropValue<C, S, F> {
    fn checked_div(self, rhs: Self) -> Option<Self> {
        self.checked_const(rhs, C::checked_div)
    }
}

impl<C: CheckedRem, S, F> CheckedRem for ConstPropValue<C, S, F> {
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        self.checked_const(rhs, C::checked_rem)
    }
}

/// An out-of-range shift amount yields `Top` rather than an analysis error.
impl<C: CheckedShl, S, F> CheckedShl for ConstPropValue<C, S, F> {
    fn checked_shl(self, rhs: Self) -> Option<Self> {
        self.checked_const(rhs, C::checked_shl).or(Some(Self::Top))
    }
}

/// An out-of-range shift amount yields `Top` rather than an analysis error.
impl<C: CheckedShr, S, F> CheckedShr for ConstPropValue<C, S, F> {
    fn checked_shr(self, rhs: Self) -> Option<Self> {
        self.checked_const(rhs, C::checked_shr).or(Some(Self::Top))
    }
}

impl<C: kirin_scf::ForLoopValue, S, F> kirin_scf::ForLoopValue for ConstPropValue<C, S, F> {
    fn loop_condition(&self, end: &Self) -> Option<bool> {
        match (self, end) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.loop_condition(rhs),
            _ => None,
        }
    }

    fn loop_step(&self, step: &Self) -> Option<Self> {
        match (self, step) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.loop_step(rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}
