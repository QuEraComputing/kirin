use std::ops::{Add, Mul, Neg, Sub};

use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{
    Continuation, Interpretable, Interpreter, InterpreterError, InterpreterExt,
};

use crate::{Arith, CheckedDiv, CheckedRem};

#[derive(Debug)]
struct DivisionByZero;

impl std::fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

impl<'ir, I, T> Interpretable<'ir, I> for Arith<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + CheckedDiv
        + CheckedRem
        + Neg<Output = I::Value>,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a + b),
            Arith::Sub {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a - b),
            Arith::Mul {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a * b),
            Arith::Div {
                lhs, rhs, result, ..
            } => interp.try_binary_op(*lhs, *rhs, *result, |a, b| {
                a.checked_div(b)
                    .ok_or_else(|| InterpreterError::custom(DivisionByZero).into())
            }),
            Arith::Rem {
                lhs, rhs, result, ..
            } => interp.try_binary_op(*lhs, *rhs, *result, |a, b| {
                a.checked_rem(b)
                    .ok_or_else(|| InterpreterError::custom(DivisionByZero).into())
            }),
            Arith::Neg {
                operand, result, ..
            } => interp.unary_op(*operand, *result, |a| -a),
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
