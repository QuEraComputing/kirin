use std::iter::{Once, once};

use crate::ir::{Instruction, ResultValue, SSAValue};

#[derive(Clone, Debug, PartialEq)]
pub enum ArithInstruction {
    Add(SSAValue, SSAValue, ResultValue),
    Sub(SSAValue, SSAValue, ResultValue),
    Mul(SSAValue, SSAValue, ResultValue),
    Div(SSAValue, SSAValue, ResultValue),
}

impl Instruction for ArithInstruction {
    type ResultIterator = Once<ResultValue>;
    fn results(&self) -> Self::ResultIterator {
        match self {
            ArithInstruction::Add(_, _, result) => once(*result),
            ArithInstruction::Sub(_, _, result) => once(*result),
            ArithInstruction::Mul(_, _, result) => once(*result),
            ArithInstruction::Div(_, _, result) => once(*result),
        }
    }
}
