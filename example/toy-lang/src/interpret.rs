use kirin::prelude::*;
use kirin_constant::Constant;
use kirin_function::{Lexical, Lifted};
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError, SSACFGRegion,
};
use kirin_scf::ForLoopValue;
use kirin_scf::StructuredControlFlow;

use crate::language::{HighLevel, LowLevel};

// ---------------------------------------------------------------------------
// HighLevel: Interpretable
// ---------------------------------------------------------------------------

impl<'ir, I> Interpretable<'ir, I, HighLevel> for HighLevel
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
    I::StageInfo: HasStageInfo<HighLevel>,
    I::Error: From<InterpreterError>,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            HighLevel::Lexical(inner) => {
                <Lexical<_> as Interpretable<'ir, I, HighLevel>>::interpret(inner, interp)
            }
            HighLevel::Structured(inner) => {
                <StructuredControlFlow<_> as Interpretable<'ir, I, HighLevel>>::interpret(
                    inner, interp,
                )
            }
            HighLevel::Constant(inner) => {
                <Constant<_, _> as Interpretable<'ir, I, HighLevel>>::interpret(inner, interp)
            }
            HighLevel::Arith(inner) => {
                <kirin_arith::Arith<_> as Interpretable<'ir, I, HighLevel>>::interpret(
                    inner, interp,
                )
            }
            HighLevel::Cmp(inner) => {
                <kirin_cmp::Cmp<_> as Interpretable<'ir, I, HighLevel>>::interpret(inner, interp)
            }
            HighLevel::Bitwise(inner) => {
                <kirin_bitwise::Bitwise<_> as Interpretable<'ir, I, HighLevel>>::interpret(
                    inner, interp,
                )
            }
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

impl<'ir, I> Interpretable<'ir, I, LowLevel> for LowLevel
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
    I::StageInfo: HasStageInfo<LowLevel>,
    I::Error: From<InterpreterError>,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            LowLevel::Lifted(inner) => {
                <Lifted<_> as Interpretable<'ir, I, LowLevel>>::interpret(inner, interp)
            }
            LowLevel::Constant(inner) => {
                <Constant<_, _> as Interpretable<'ir, I, LowLevel>>::interpret(inner, interp)
            }
            LowLevel::Arith(inner) => {
                <kirin_arith::Arith<_> as Interpretable<'ir, I, LowLevel>>::interpret(inner, interp)
            }
            LowLevel::Cmp(inner) => {
                <kirin_cmp::Cmp<_> as Interpretable<'ir, I, LowLevel>>::interpret(inner, interp)
            }
            LowLevel::Bitwise(inner) => {
                <kirin_bitwise::Bitwise<_> as Interpretable<'ir, I, LowLevel>>::interpret(
                    inner, interp,
                )
            }
            LowLevel::Cf(inner) => {
                <kirin_cf::ControlFlow<_> as Interpretable<'ir, I, LowLevel>>::interpret(
                    inner, interp,
                )
            }
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
