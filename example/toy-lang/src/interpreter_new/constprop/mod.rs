mod ops;

use kirin::ir::{HasBottom, HasTop, Lattice, Product};
use kirin_arith::ArithValue;
use kirin_interpreter_new::{BranchCondition, HasProductValue};
use kirin_scf::ForLoopValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstProp {
    Bottom,
    Const(i64),
    Product(Box<Product<Self>>),
    Top,
}

impl Lattice for ConstProp {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(*lhs),
            (Self::Product(lhs), Self::Product(rhs)) if lhs.len() == rhs.len() => Self::Product(
                Box::new(lhs.iter().zip(rhs.iter()).map(|(l, r)| l.join(r)).collect()),
            ),
            _ => Self::Top,
        }
    }

    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Top, value) | (value, Self::Top) => value.clone(),
            (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(*lhs),
            (Self::Product(lhs), Self::Product(rhs)) if lhs.len() == rhs.len() => Self::Product(
                Box::new(lhs.iter().zip(rhs.iter()).map(|(l, r)| l.meet(r)).collect()),
            ),
            _ => Self::Bottom,
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self.join(other) == *other
    }
}

impl HasBottom for ConstProp {
    fn bottom() -> Self {
        Self::Bottom
    }
}

impl HasTop for ConstProp {
    fn top() -> Self {
        Self::Top
    }
}

impl BranchCondition for ConstProp {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            Self::Const(0) => Some(false),
            Self::Const(_) => Some(true),
            Self::Bottom | Self::Product(_) | Self::Top => None,
        }
    }
}

impl HasProductValue for ConstProp {
    fn from_product(product: Product<Self>) -> Self {
        Self::Product(Box::new(product))
    }

    fn as_product(&self) -> Option<&Product<Self>> {
        match self {
            Self::Product(product) => Some(product.as_ref()),
            _ => None,
        }
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
