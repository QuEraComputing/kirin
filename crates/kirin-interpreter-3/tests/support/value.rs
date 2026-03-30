#![allow(dead_code)]

use std::fmt::{Display, Formatter};

use kirin_interpreter_3::{BranchCondition, ProductValue};
use kirin_ir::{Placeholder, Product};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum TestType {
    I64,
    Tuple,
}

impl Display for TestType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I64 => write!(f, "i64"),
            Self::Tuple => write!(f, "tuple"),
        }
    }
}

impl Placeholder for TestType {
    fn placeholder() -> Self {
        Self::I64
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum TestValue {
    I64(i64),
    Bool(bool),
    Unknown,
    Tuple(Box<Product<Self>>),
}

impl From<i64> for TestValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<bool> for TestValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl ProductValue for TestValue {
    fn as_product(&self) -> Option<&Product<Self>> {
        match self {
            Self::Tuple(product) => Some(product.as_ref()),
            Self::I64(_) | Self::Bool(_) | Self::Unknown => None,
        }
    }

    fn from_product(product: Product<Self>) -> Self {
        Self::Tuple(Box::new(product))
    }
}

impl BranchCondition for TestValue {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            Self::I64(value) => Some(*value != 0),
            Self::Bool(value) => Some(*value),
            Self::Unknown => None,
            Self::Tuple(_) => None,
        }
    }
}
