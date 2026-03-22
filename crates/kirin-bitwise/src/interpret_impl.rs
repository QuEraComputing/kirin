use std::ops::{BitAnd, BitOr, BitXor, Not};

use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{Interpretable, Interpreter, InterpreterError, InterpreterExt};

use crate::{Bitwise, CheckedShl, CheckedShr};

#[derive(Debug)]
struct ShiftOverflow;

impl std::fmt::Display for ShiftOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shift amount out of range")
    }
}

impl std::error::Error for ShiftOverflow {}

impl<'ir, I, T> Interpretable<'ir, I> for Bitwise<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + CheckedShl
        + CheckedShr,
    T: CompileTimeValue,
{
    fn interpret<L>(
        &self,
        interp: &mut I,
    ) -> Result<kirin_interpreter::Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            Bitwise::And {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a & b),
            Bitwise::Or {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a | b),
            Bitwise::Xor {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a ^ b),
            Bitwise::Not {
                operand, result, ..
            } => interp.unary_op(*operand, *result, |a| !a),
            Bitwise::Shl {
                lhs, rhs, result, ..
            } => interp.try_binary_op(*lhs, *rhs, *result, |a, b| {
                a.checked_shl(b)
                    .ok_or_else(|| InterpreterError::custom(ShiftOverflow).into())
            }),
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => interp.try_binary_op(*lhs, *rhs, *result, |a, b| {
                a.checked_shr(b)
                    .ok_or_else(|| InterpreterError::custom(ShiftOverflow).into())
            }),
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
