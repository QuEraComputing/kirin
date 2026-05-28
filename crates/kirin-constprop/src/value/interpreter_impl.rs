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

impl<S, F> BranchCondition for ConstPropValue<i64, S, F>
where
    S: Clone,
    F: Clone,
{
    fn is_truthy(&self) -> Option<bool> {
        match self {
            Self::Const(0) => Some(false),
            Self::Const(_) => Some(true),
            Self::Bottom | Self::PartialTuple(_) | Self::PartialStruct(_) | Self::Top => None,
        }
    }
}

impl<S, F> From<ArithValue> for ConstPropValue<i64, S, F> {
    fn from(value: ArithValue) -> Self {
        match value {
            ArithValue::I64(value) => Self::Const(value),
            _ => Self::Top,
        }
    }
}
