use crate::SimpleType;
use kirin_ir::{Dialect, ResultValue, SSAValue};

/// Operands with `#[kirin(type = ...)]`, so the generated builders check them.
#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(builders, type = SimpleType, crate = kirin_ir)]
pub enum TypedOperandLanguage {
    /// The first operand must be `I64`. The second has no annotation and is never checked.
    Index(
        #[kirin(type = SimpleType::I64)] SSAValue,
        SSAValue,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    /// The expected type comes from another field (`SimpleType` isn't `Copy`).
    Cast {
        #[kirin(type = target.clone())]
        input: SSAValue,
        target: SimpleType,
        #[kirin(type = target.clone())]
        result: ResultValue,
    },
}
