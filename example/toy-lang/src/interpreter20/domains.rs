use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::Lattice;
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_scf::ForLoopValue;

// ---------------------------------------------------------------------------
// ToyType — type lattice for abstract interpretation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToyType {
    Bottom,
    I64,
    Bool,
    Top,
}

impl kirin::prelude::Lattice for ToyType {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (ToyType::Bottom, x) | (x, ToyType::Bottom) => x.clone(),
            (a, b) if a == b => a.clone(),
            _ => ToyType::Top,
        }
    }
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (ToyType::Top, x) | (x, ToyType::Top) => x.clone(),
            (a, b) if a == b => a.clone(),
            _ => ToyType::Bottom,
        }
    }
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (_, ToyType::Top) | (ToyType::Bottom, _)) || self == other
    }
}

impl kirin::prelude::HasBottom for ToyType {
    fn bottom() -> Self {
        ToyType::Bottom
    }
}

impl AbstractValue for ToyType {
    fn widen(&self, next: &Self) -> Self {
        self.join(next)
    }
}

impl BranchCondition for ToyType {
    fn is_truthy(&self) -> Option<bool> {
        None
    }
}

impl ForLoopValue for ToyType {
    fn loop_condition(&self, _end: &Self) -> Option<bool> {
        None
    }
    fn loop_step(&self, _step: &Self) -> Option<Self> {
        Some(self.join(_step))
    }
}

impl ProductValue for ToyType {
    fn as_product(&self) -> Option<&kirin::prelude::Product<Self>> {
        None
    }
    fn from_product(_product: kirin::prelude::Product<Self>) -> Self {
        ToyType::Top
    }
}

impl From<ArithValue> for ToyType {
    fn from(_: ArithValue) -> Self {
        ToyType::I64
    }
}

impl CompareValue for ToyType {
    type Bool = ToyType;
    fn cmp_eq(&self, _: &Self) -> ToyType {
        ToyType::Bool
    }
    fn cmp_ne(&self, _: &Self) -> ToyType {
        ToyType::Bool
    }
    fn cmp_lt(&self, _: &Self) -> ToyType {
        ToyType::Bool
    }
    fn cmp_le(&self, _: &Self) -> ToyType {
        ToyType::Bool
    }
    fn cmp_gt(&self, _: &Self) -> ToyType {
        ToyType::Bool
    }
    fn cmp_ge(&self, _: &Self) -> ToyType {
        ToyType::Bool
    }
}

impl Add for ToyType {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.join(&rhs)
    }
}
impl Sub for ToyType {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.join(&rhs)
    }
}
impl Mul for ToyType {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.join(&rhs)
    }
}
impl Neg for ToyType {
    type Output = Self;
    fn neg(self) -> Self {
        self
    }
}
impl Not for ToyType {
    type Output = Self;
    fn not(self) -> Self {
        self
    }
}
impl BitAnd for ToyType {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.join(&rhs)
    }
}
impl BitOr for ToyType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.join(&rhs)
    }
}
impl BitXor for ToyType {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        self.join(&rhs)
    }
}
impl CheckedDiv for ToyType {
    fn checked_div(self, _: Self) -> Option<Self> {
        Some(self)
    }
}
impl CheckedRem for ToyType {
    fn checked_rem(self, _: Self) -> Option<Self> {
        Some(self)
    }
}
impl CheckedShl for ToyType {
    fn checked_shl(self, _: Self) -> Option<Self> {
        Some(self)
    }
}
impl CheckedShr for ToyType {
    fn checked_shr(self, _: Self) -> Option<Self> {
        Some(self)
    }
}

// ---------------------------------------------------------------------------
// ConstProp — extensibility probe domain (R8)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstProp {
    Bottom,
    Const(i64),
    Top,
}

impl kirin::prelude::Lattice for ConstProp {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (ConstProp::Bottom, x) | (x, ConstProp::Bottom) => x.clone(),
            (ConstProp::Const(a), ConstProp::Const(b)) if a == b => ConstProp::Const(*a),
            _ => ConstProp::Top,
        }
    }
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (ConstProp::Top, x) | (x, ConstProp::Top) => x.clone(),
            (ConstProp::Const(a), ConstProp::Const(b)) if a == b => ConstProp::Const(*a),
            _ => ConstProp::Bottom,
        }
    }
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (_, ConstProp::Top) | (ConstProp::Bottom, _)) || self == other
    }
}

impl kirin::prelude::HasBottom for ConstProp {
    fn bottom() -> Self {
        ConstProp::Bottom
    }
}

impl AbstractValue for ConstProp {
    fn widen(&self, next: &Self) -> Self {
        self.join(next)
    }
}

impl BranchCondition for ConstProp {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            ConstProp::Const(0) => Some(false),
            ConstProp::Const(_) => Some(true),
            _ => None,
        }
    }
}

impl ForLoopValue for ConstProp {
    fn loop_condition(&self, end: &Self) -> Option<bool> {
        match (self, end) {
            (ConstProp::Const(iv), ConstProp::Const(e)) => Some(iv < e),
            _ => None,
        }
    }
    fn loop_step(&self, step: &Self) -> Option<Self> {
        match (self, step) {
            (ConstProp::Const(iv), ConstProp::Const(s)) => {
                Some(ConstProp::Const(iv.wrapping_add(*s)))
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
            _ => Some(ConstProp::Top),
        }
    }
}

impl ProductValue for ConstProp {
    fn as_product(&self) -> Option<&kirin::prelude::Product<Self>> {
        None
    }
    fn from_product(_: kirin::prelude::Product<Self>) -> Self {
        ConstProp::Top
    }
}

impl From<ArithValue> for ConstProp {
    fn from(v: ArithValue) -> Self {
        match v {
            ArithValue::I64(n) => ConstProp::Const(n),
            _ => ConstProp::Top,
        }
    }
}

impl CompareValue for ConstProp {
    type Bool = ConstProp;
    fn cmp_eq(&self, other: &Self) -> ConstProp {
        match (self, other) {
            (ConstProp::Const(a), ConstProp::Const(b)) => {
                ConstProp::Const(if a == b { 1 } else { 0 })
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
    fn cmp_ne(&self, other: &Self) -> ConstProp {
        match (self, other) {
            (ConstProp::Const(a), ConstProp::Const(b)) => {
                ConstProp::Const(if a != b { 1 } else { 0 })
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
    fn cmp_lt(&self, other: &Self) -> ConstProp {
        match (self, other) {
            (ConstProp::Const(a), ConstProp::Const(b)) => {
                ConstProp::Const(if a < b { 1 } else { 0 })
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
    fn cmp_le(&self, other: &Self) -> ConstProp {
        match (self, other) {
            (ConstProp::Const(a), ConstProp::Const(b)) => {
                ConstProp::Const(if a <= b { 1 } else { 0 })
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
    fn cmp_gt(&self, other: &Self) -> ConstProp {
        match (self, other) {
            (ConstProp::Const(a), ConstProp::Const(b)) => {
                ConstProp::Const(if a > b { 1 } else { 0 })
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
    fn cmp_ge(&self, other: &Self) -> ConstProp {
        match (self, other) {
            (ConstProp::Const(a), ConstProp::Const(b)) => {
                ConstProp::Const(if a >= b { 1 } else { 0 })
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}

impl Add for ConstProp {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a.wrapping_add(b)),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}
impl Sub for ConstProp {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a.wrapping_sub(b)),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}
impl Mul for ConstProp {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a.wrapping_mul(b)),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}
impl Neg for ConstProp {
    type Output = Self;
    fn neg(self) -> Self {
        match self {
            ConstProp::Const(n) => ConstProp::Const(n.wrapping_neg()),
            other => other,
        }
    }
}
impl Not for ConstProp {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            ConstProp::Const(n) => ConstProp::Const(!n),
            other => other,
        }
    }
}
impl BitAnd for ConstProp {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a & b),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}
impl BitOr for ConstProp {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a | b),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}
impl BitXor for ConstProp {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a ^ b),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
            _ => ConstProp::Top,
        }
    }
}
impl CheckedDiv for ConstProp {
    fn checked_div(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => a.checked_div(b).map(ConstProp::Const),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
            _ => Some(ConstProp::Top),
        }
    }
}
impl CheckedRem for ConstProp {
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) => a.checked_rem(b).map(ConstProp::Const),
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
            _ => Some(ConstProp::Top),
        }
    }
}
impl CheckedShl for ConstProp {
    fn checked_shl(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) if (0..64i64).contains(&b) => {
                Some(ConstProp::Const(a << b))
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
            _ => Some(ConstProp::Top),
        }
    }
}
impl CheckedShr for ConstProp {
    fn checked_shr(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (ConstProp::Const(a), ConstProp::Const(b)) if (0..64i64).contains(&b) => {
                Some(ConstProp::Const(a >> b))
            }
            (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
            _ => Some(ConstProp::Top),
        }
    }
}
