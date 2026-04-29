mod ops;

use kirin_arith::ArithValue;
use kirin_interpreter_new::{AbstractValue, BranchCondition, ProductValue};
use kirin_scf::ForLoopValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstProp {
    Bottom,
    Const(i64),
    Top,
}

impl AbstractValue for ConstProp {
    fn bottom() -> Self {
        Self::Bottom
    }

    fn top() -> Self {
        Self::Top
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(*lhs),
            _ => Self::Top,
        }
    }
}

impl BranchCondition for ConstProp {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            Self::Const(0) => Some(false),
            Self::Const(_) => Some(true),
            Self::Bottom | Self::Top => None,
        }
    }
}

impl ProductValue for ConstProp {
    fn new_product(values: Vec<Self>) -> Self {
        match values.as_slice() {
            [value] => value.clone(),
            _ => Self::Top,
        }
    }

    fn as_product(&self) -> Option<&[Self]> {
        None
    }
}

impl ForLoopValue for ConstProp {
    fn loop_condition(&self, end: &Self) -> Option<bool> {
        match (self, end) {
            (Self::Const(lhs), Self::Const(rhs)) => Some(lhs < rhs),
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

impl From<ArithValue> for ConstProp {
    fn from(value: ArithValue) -> Self {
        match value {
            ArithValue::I64(value) => Self::Const(value),
            _ => Self::Top,
        }
    }
}
