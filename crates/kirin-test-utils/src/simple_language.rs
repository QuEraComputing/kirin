use crate::{SimpleIRType, Value};
use kirin_ir::{Dialect, Region, ResultValue, SSAKind, SSAValue};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(fn, type = SimpleIRType, crate = kirin_ir)]
pub enum SimpleLanguage {
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleIRType::Float)] ResultValue,
    ),
    Constant(
        #[kirin(into)] Value,
        #[kirin(type = SimpleIRType::Float)] ResultValue,
    ),
    #[kirin(terminator)]
    Return(SSAValue),
    Function(Region, #[kirin(type = SimpleIRType::Float)] ResultValue),
}
