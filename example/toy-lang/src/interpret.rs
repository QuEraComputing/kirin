use kirin::prelude::*;
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError, SSACFGRegion,
};
use kirin_scf::ForLoopValue;

use crate::language::{HighLevel, LowLevel};

// ---------------------------------------------------------------------------
// HighLevel: Interpretable
// ---------------------------------------------------------------------------

impl<'ir, I> Interpretable<'ir, I> for HighLevel
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + std::ops::Add<Output = I::Value>
        + std::ops::Sub<Output = I::Value>
        + std::ops::Mul<Output = I::Value>
        + kirin_arith::CheckedDiv
        + kirin_arith::CheckedRem
        + std::ops::Neg<Output = I::Value>
        + kirin_cmp::CompareValue
        + std::ops::BitAnd<Output = I::Value>
        + std::ops::BitOr<Output = I::Value>
        + std::ops::BitXor<Output = I::Value>
        + std::ops::Not<Output = I::Value>
        + std::ops::Shl<Output = I::Value>
        + std::ops::Shr<Output = I::Value>
        + BranchCondition
        + ForLoopValue
        + From<kirin_arith::ArithValue>,
    I::Error: From<InterpreterError>,
{
    fn interpret<L: Dialect>(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            HighLevel::Lexical(inner) => inner.interpret::<L>(interp),
            HighLevel::Structured(inner) => inner.interpret::<L>(interp),
            HighLevel::Constant(inner) => inner.interpret::<L>(interp),
            HighLevel::Arith(inner) => inner.interpret::<L>(interp),
            HighLevel::Cmp(inner) => inner.interpret::<L>(interp),
            HighLevel::Bitwise(inner) => inner.interpret::<L>(interp),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevel: SSACFGRegion (provides blanket CallSemantics)
// ---------------------------------------------------------------------------

impl SSACFGRegion for HighLevel {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        match self {
            HighLevel::Lexical(inner) => inner.entry_block(stage),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable
// ---------------------------------------------------------------------------

impl<'ir, I> Interpretable<'ir, I> for LowLevel
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + std::ops::Add<Output = I::Value>
        + std::ops::Sub<Output = I::Value>
        + std::ops::Mul<Output = I::Value>
        + kirin_arith::CheckedDiv
        + kirin_arith::CheckedRem
        + std::ops::Neg<Output = I::Value>
        + kirin_cmp::CompareValue
        + std::ops::BitAnd<Output = I::Value>
        + std::ops::BitOr<Output = I::Value>
        + std::ops::BitXor<Output = I::Value>
        + std::ops::Not<Output = I::Value>
        + std::ops::Shl<Output = I::Value>
        + std::ops::Shr<Output = I::Value>
        + BranchCondition
        + From<kirin_arith::ArithValue>,
    I::Error: From<InterpreterError>,
{
    fn interpret<L: Dialect>(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            LowLevel::Lifted(inner) => inner.interpret::<L>(interp),
            LowLevel::Constant(inner) => inner.interpret::<L>(interp),
            LowLevel::Arith(inner) => inner.interpret::<L>(interp),
            LowLevel::Cmp(inner) => inner.interpret::<L>(interp),
            LowLevel::Bitwise(inner) => inner.interpret::<L>(interp),
            LowLevel::Cf(inner) => inner.interpret::<L>(interp),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: SSACFGRegion (provides blanket CallSemantics)
// ---------------------------------------------------------------------------

impl SSACFGRegion for LowLevel {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        match self {
            LowLevel::Lifted(inner) => inner.entry_block(stage),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}
