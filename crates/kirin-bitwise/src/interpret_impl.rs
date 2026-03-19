use std::ops::{BitAnd, BitOr, BitXor, Not, Shl, Shr};

use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{Interpretable, Interpreter, InterpreterError, InterpreterExt};

use crate::Bitwise;

impl<'ir, I, T> Interpretable<'ir, I> for Bitwise<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + Shl<Output = I::Value>
        + Shr<Output = I::Value>,
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
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a << b),
            Bitwise::Shr {
                lhs, rhs, result, ..
            } => interp.binary_op(*lhs, *rhs, *result, |a, b| a >> b),
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
