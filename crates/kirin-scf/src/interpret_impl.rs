use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, Successor};
use kirin_interpreter::smallvec::smallvec;
use kirin_interpreter::{
    BlockExecutor, BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError,
};

use crate::{For, If, StructuredControlFlow, Yield};

/// Trait for values that can serve as induction variables in `scf.for` loops.
pub trait ForLoopValue {
    /// Returns whether the loop should continue (`self < end`).
    fn loop_condition(&self, end: &Self) -> Option<bool>;
    /// Advance the induction variable by `step`.
    fn loop_step(&self, step: &Self) -> Self;
}

impl ForLoopValue for i64 {
    fn loop_condition(&self, end: &i64) -> Option<bool> {
        Some(*self < *end)
    }

    fn loop_step(&self, step: &i64) -> i64 {
        self + step
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for If<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + BranchCondition,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let cond = interp.read(self.condition)?;
        let then_target = Successor::from_block(self.then_body);
        let else_target = Successor::from_block(self.else_body);
        match cond.is_truthy() {
            Some(true) => Ok(Continuation::Jump(then_target, smallvec![])),
            Some(false) => Ok(Continuation::Jump(else_target, smallvec![])),
            None => Ok(Continuation::Fork(smallvec![
                (then_target, smallvec![]),
                (else_target, smallvec![]),
            ])),
        }
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for For<T>
where
    I: Interpreter<'ir> + BlockExecutor<'ir, L>,
    I::Value: Clone + ForLoopValue,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let mut iv = interp.read(self.start)?;
        let end = interp.read(self.end)?;
        let step = interp.read(self.step)?;
        let stage = interp.active_stage_info::<L>();
        while iv.loop_condition(&end) == Some(true) {
            interp.bind_block_args(stage, self.body, &[iv.clone()])?;
            let control = interp.eval_block(stage, self.body)?;
            match control {
                Continuation::Yield(_) => {}
                other => return Ok(other),
            }
            iv = iv.loop_step(&step);
        }
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for Yield<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let v = interp.read(self.value)?;
        Ok(Continuation::Yield(v))
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for StructuredControlFlow<T>
where
    I: Interpreter<'ir> + BlockExecutor<'ir, L>,
    I::Value: Clone + BranchCondition + ForLoopValue,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            StructuredControlFlow::If(op) => {
                <If<T> as Interpretable<'ir, I, L>>::interpret(op, interp)
            }
            StructuredControlFlow::For(op) => {
                <For<T> as Interpretable<'ir, I, L>>::interpret(op, interp)
            }
            StructuredControlFlow::Yield(op) => {
                <Yield<T> as Interpretable<'ir, I, L>>::interpret(op, interp)
            }
        }
    }
}
