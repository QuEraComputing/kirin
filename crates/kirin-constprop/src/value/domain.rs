use super::{PartialStruct, PartialTuple};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstPropValue<C = i64, S = String, F = String> {
    Bottom,
    Const(C),
    PartialTuple(Box<PartialTuple<Self>>),
    PartialStruct(Box<PartialStruct<S, F, Self>>),
    Top,
}

impl<C, S, F> ConstPropValue<C, S, F> {
    pub fn tuple(elements: kirin_ir::Product<Self>) -> Self {
        Self::PartialTuple(Box::new(PartialTuple::new(elements)))
    }

    pub fn struct_value(shape: S, fields: Vec<(F, Self)>) -> Self {
        Self::PartialStruct(Box::new(PartialStruct::new(shape, fields)))
    }

    pub fn as_const(&self) -> Option<&C> {
        match self {
            Self::Const(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_tuple(&self) -> Option<&PartialTuple<Self>> {
        match self {
            Self::PartialTuple(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&PartialStruct<S, F, Self>> {
        match self {
            Self::PartialStruct(value) => Some(value),
            _ => None,
        }
    }
}

impl<S, F> From<i64> for ConstPropValue<i64, S, F> {
    fn from(value: i64) -> Self {
        Self::Const(value)
    }
}
