use kirin::prelude::*;
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError, SSACFGRegion,
};
use kirin_scf::ForLoopValue;
use smallvec::smallvec;

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
            HighLevel::Function { body } => {
                let stage = interp.resolve_stage::<HighLevel>()?;
                let entry = body
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())?;
                Ok(Continuation::Jump(entry, smallvec![]))
            }
            HighLevel::Lambda { body, .. } => {
                let stage = interp.resolve_stage::<HighLevel>()?;
                let entry = body
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())?;
                Ok(Continuation::Jump(entry, smallvec![]))
            }
            HighLevel::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond = interp.read(*condition)?;
                match cond.is_truthy() {
                    Some(true) => Ok(Continuation::Jump(*then_body, smallvec![])),
                    Some(false) => Ok(Continuation::Jump(*else_body, smallvec![])),
                    None => Ok(Continuation::Fork(smallvec![
                        (*then_body, smallvec![]),
                        (*else_body, smallvec![]),
                    ])),
                }
            }
            HighLevel::For {
                start,
                end,
                step,
                body,
                ..
            } => {
                let mut iv = interp.read(*start)?;
                let end_val = interp.read(*end)?;
                let step_val = interp.read(*step)?;
                let stage = interp.active_stage_info::<HighLevel>();
                while iv.loop_condition(&end_val) == Some(true) {
                    interp.bind_block_args(stage, *body, &[iv.clone()])?;
                    let control = interp.eval_block(stage, *body)?;
                    match control {
                        Continuation::Yield(_) => {}
                        other => return Ok(other),
                    }
                    iv = iv.loop_step(&step_val);
                }
                Ok(Continuation::Continue)
            }
            HighLevel::Yield { value } => {
                let v = interp.read(*value)?;
                Ok(Continuation::Yield(v))
            }
            HighLevel::Constant { value, result } => {
                let val = I::Value::from(value.clone());
                interp.write(*result, val)?;
                Ok(Continuation::Continue)
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
            HighLevel::Call(inner) => {
                <kirin_function::Call<_> as Interpretable<'ir, I, HighLevel>>::interpret(
                    inner, interp,
                )
            }
            HighLevel::Return(inner) => {
                <kirin_function::Return<_> as Interpretable<'ir, I, HighLevel>>::interpret(
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
            HighLevel::Function { body } | HighLevel::Lambda { body, .. } => body
                .blocks(stage)
                .next()
                .ok_or(InterpreterError::missing_entry_block()),
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
            LowLevel::Function { body } => {
                let stage = interp.resolve_stage::<LowLevel>()?;
                let entry = body
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())?;
                Ok(Continuation::Jump(entry, smallvec![]))
            }
            LowLevel::Constant { value, result } => {
                let val = I::Value::from(value.clone());
                interp.write(*result, val)?;
                Ok(Continuation::Continue)
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
            LowLevel::Bind(_) => Err(InterpreterError::custom(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "bind is not yet supported in the interpreter",
            ))
            .into()),
            LowLevel::Call(inner) => {
                <kirin_function::Call<_> as Interpretable<'ir, I, LowLevel>>::interpret(
                    inner, interp,
                )
            }
            LowLevel::Return(inner) => {
                <kirin_function::Return<_> as Interpretable<'ir, I, LowLevel>>::interpret(
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
            LowLevel::Function { body } => body
                .blocks(stage)
                .next()
                .ok_or(InterpreterError::missing_entry_block()),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}
