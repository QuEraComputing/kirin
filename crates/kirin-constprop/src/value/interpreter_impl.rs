use kirin_arith::ArithValue;
use kirin_interpreter::{BranchCondition, HasProductValue};
use kirin_ir::Product;

use super::ConstPropValue;

impl<C, S, F> HasProductValue for ConstPropValue<C, S, F>
where
    C: Clone,
    S: Clone,
    F: Clone,
{
    fn from_product(product: Product<Self>) -> Self {
        Self::tuple(product)
    }

    fn as_product(&self) -> Option<&Product<Self>> {
        self.as_tuple().map(|value| value.elements())
    }
}

impl<C: BranchCondition, S, F> BranchCondition for ConstPropValue<C, S, F> {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            Self::Const(value) => value.is_truthy(),
            Self::Bottom | Self::PartialTuple(_) | Self::PartialStruct(_) | Self::Top => None,
        }
    }
}

/// A literal is a constant only if converting it back gives the same literal,
/// so `1.0` never folds as the integer `1`.
impl<C, S, F> From<ArithValue> for ConstPropValue<C, S, F>
where
    C: TryFrom<ArithValue> + Into<ArithValue> + Clone,
{
    fn from(value: ArithValue) -> Self {
        match C::try_from(value.clone()) {
            Ok(constant) if constant.clone().into() == value => Self::Const(constant),
            _ => Self::Top,
        }
    }
}
