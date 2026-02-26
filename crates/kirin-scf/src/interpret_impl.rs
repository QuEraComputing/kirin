use kirin::prelude::{CompileTimeValue, Dialect, Successor};
use kirin_interpreter::smallvec::smallvec;
use kirin_interpreter::{
    BlockExecutor, BranchCondition, Continuation, Interpretable, Interpreter,
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

impl<I, L, T> Interpretable<I, L> for If<T>
where
    I: Interpreter,
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

impl<I, L, T> Interpretable<I, L> for For<T>
where
    I: Interpreter + BlockExecutor<L>,
    I::Value: Clone + ForLoopValue,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let mut iv = interp.read(self.start)?;
        let end = interp.read(self.end)?;
        let step = interp.read(self.step)?;
        while iv.loop_condition(&end) == Some(true) {
            let _yielded = interp.execute_block(self.body, &[iv.clone()])?;
            iv = iv.loop_step(&step);
        }
        Ok(Continuation::Continue)
    }
}

impl<I, L, T> Interpretable<I, L> for Yield<T>
where
    I: Interpreter,
    I::Value: Clone,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let v = interp.read(self.value)?;
        Ok(Continuation::Yield(v))
    }
}

impl<I, L, T> Interpretable<I, L> for StructuredControlFlow<T>
where
    I: Interpreter + BlockExecutor<L>,
    I::Value: Clone + BranchCondition + ForLoopValue,
    L: Dialect,
    T: CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            StructuredControlFlow::If(op) => <If<T> as Interpretable<I, L>>::interpret(op, interp),
            StructuredControlFlow::For(op) => {
                <For<T> as Interpretable<I, L>>::interpret(op, interp)
            }
            StructuredControlFlow::Yield(op) => {
                <Yield<T> as Interpretable<I, L>>::interpret(op, interp)
            }
        }
    }
}
