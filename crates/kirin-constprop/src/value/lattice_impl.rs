use kirin_ir::{HasBottom, HasTop, Lattice};

use super::ConstPropValue;

impl<C, S, F> Lattice for ConstPropValue<C, S, F>
where
    C: Clone + PartialEq,
    S: Clone + PartialEq,
    F: Clone + PartialEq,
{
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Top, _) | (_, Self::Top) => Self::Top,
            (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(lhs.clone()),
            (Self::PartialTuple(lhs), Self::PartialTuple(rhs)) => lhs
                .zip_map(rhs, Lattice::join)
                .map(|value| Self::PartialTuple(Box::new(value)))
                .unwrap_or(Self::Top),
            (Self::PartialStruct(lhs), Self::PartialStruct(rhs)) => lhs
                .zip_map(rhs, Lattice::join)
                .map(|value| Self::PartialStruct(Box::new(value)))
                .unwrap_or(Self::Top),
            _ => Self::Top,
        }
    }

    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Top, value) | (value, Self::Top) => value.clone(),
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(lhs.clone()),
            (Self::PartialTuple(lhs), Self::PartialTuple(rhs)) => lhs
                .zip_map(rhs, Lattice::meet)
                .map(|value| Self::PartialTuple(Box::new(value)))
                .unwrap_or(Self::Bottom),
            (Self::PartialStruct(lhs), Self::PartialStruct(rhs)) => lhs
                .zip_map(rhs, Lattice::meet)
                .map(|value| Self::PartialStruct(Box::new(value)))
                .unwrap_or(Self::Bottom),
            _ => Self::Bottom,
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self.join(other) == *other
    }
}

impl<C, S, F> HasBottom for ConstPropValue<C, S, F>
where
    C: Clone + PartialEq,
    S: Clone + PartialEq,
    F: Clone + PartialEq,
{
    fn bottom() -> Self {
        Self::Bottom
    }
}

impl<C, S, F> HasTop for ConstPropValue<C, S, F>
where
    C: Clone + PartialEq,
    S: Clone + PartialEq,
    F: Clone + PartialEq,
{
    fn top() -> Self {
        Self::Top
    }
}

/// `ConstPropValue` widens by joining: the flat constant lattice has finite
/// height, so plain joins already terminate.
impl<C, S, F> kirin_ir::Widen for ConstPropValue<C, S, F>
where
    C: Clone + PartialEq,
    S: Clone + PartialEq,
    F: Clone + PartialEq,
{
    fn widen(&self, other: &Self) -> Self {
        self.join(other)
    }
}
