use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo};
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError,
};
use smallvec::smallvec;

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

#[cfg(test)]
mod tests {
    use super::*;

    // --- ForLoopValue::loop_condition ---

    #[test]
    fn loop_condition_less_than_end() {
        assert_eq!(0i64.loop_condition(&10), Some(true));
    }

    #[test]
    fn loop_condition_equal_to_end() {
        assert_eq!(10i64.loop_condition(&10), Some(false));
    }

    #[test]
    fn loop_condition_greater_than_end() {
        assert_eq!(15i64.loop_condition(&10), Some(false));
    }

    #[test]
    fn loop_condition_negative_range() {
        assert_eq!((-5i64).loop_condition(&-1), Some(true));
        assert_eq!((-1i64).loop_condition(&-5), Some(false));
    }

    #[test]
    fn loop_condition_at_zero() {
        assert_eq!(0i64.loop_condition(&0), Some(false));
        assert_eq!((-1i64).loop_condition(&0), Some(true));
        assert_eq!(0i64.loop_condition(&1), Some(true));
    }

    #[test]
    fn loop_condition_i64_boundaries() {
        assert_eq!(i64::MIN.loop_condition(&i64::MAX), Some(true));
        assert_eq!(i64::MAX.loop_condition(&i64::MIN), Some(false));
        assert_eq!(i64::MAX.loop_condition(&i64::MAX), Some(false));
    }

    // --- ForLoopValue::loop_step ---

    #[test]
    fn loop_step_positive() {
        assert_eq!(0i64.loop_step(&1), 1);
        assert_eq!(5i64.loop_step(&3), 8);
    }

    #[test]
    fn loop_step_negative() {
        assert_eq!(10i64.loop_step(&-1), 9);
        assert_eq!(0i64.loop_step(&-5), -5);
    }

    #[test]
    fn loop_step_zero() {
        assert_eq!(42i64.loop_step(&0), 42);
    }

    #[test]
    fn loop_step_from_negative() {
        assert_eq!((-10i64).loop_step(&3), -7);
    }

    // --- Simulate a complete loop ---

    #[test]
    fn simulate_loop_zero_to_five() {
        let mut iv = 0i64;
        let end = 5i64;
        let step = 1i64;
        let mut iterations = 0;
        while iv.loop_condition(&end) == Some(true) {
            iterations += 1;
            iv = iv.loop_step(&step);
        }
        assert_eq!(iterations, 5);
        assert_eq!(iv, 5);
    }

    #[test]
    fn simulate_loop_step_two() {
        let mut iv = 0i64;
        let end = 10i64;
        let step = 2i64;
        let mut iterations = 0;
        while iv.loop_condition(&end) == Some(true) {
            iterations += 1;
            iv = iv.loop_step(&step);
        }
        assert_eq!(iterations, 5);
        assert_eq!(iv, 10);
    }

    #[test]
    fn simulate_loop_empty_range() {
        let iv = 10i64;
        let end = 5i64;
        let mut iterations = 0;
        let mut current = iv;
        while current.loop_condition(&end) == Some(true) {
            iterations += 1;
            current = current.loop_step(&1);
        }
        assert_eq!(iterations, 0);
    }

    #[test]
    fn simulate_loop_single_iteration() {
        let mut iv = 0i64;
        let end = 1i64;
        let step = 1i64;
        let mut iterations = 0;
        while iv.loop_condition(&end) == Some(true) {
            iterations += 1;
            iv = iv.loop_step(&step);
        }
        assert_eq!(iterations, 1);
    }

    // --- loop_condition always returns Some for i64 ---

    #[test]
    fn loop_condition_always_some() {
        // The i64 implementation is concrete, never returns None
        assert!(0i64.loop_condition(&0).is_some());
        assert!(i64::MIN.loop_condition(&i64::MAX).is_some());
        assert!(i64::MAX.loop_condition(&i64::MIN).is_some());
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
        match cond.is_truthy() {
            Some(true) => Ok(Continuation::Jump(self.then_body, smallvec![])),
            Some(false) => Ok(Continuation::Jump(self.else_body, smallvec![])),
            None => Ok(Continuation::Fork(smallvec![
                (self.then_body, smallvec![]),
                (self.else_body, smallvec![]),
            ])),
        }
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for For<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ForLoopValue,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect + Interpretable<'ir, I, L> + 'ir,
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
    I: Interpreter<'ir>,
    I::Value: Clone + BranchCondition + ForLoopValue,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect + Interpretable<'ir, I, L> + 'ir,
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
