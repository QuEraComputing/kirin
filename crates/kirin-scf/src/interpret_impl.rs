use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError, ProductValue,
};
use smallvec::smallvec;

use crate::{For, If, StructuredControlFlow, Yield};

/// Trait for values that can serve as induction variables in `scf.for` loops.
pub trait ForLoopValue {
    /// Returns whether the loop should continue (`self < end`).
    ///
    /// Returns `None` when the loop condition is indeterminate. For concrete
    /// interpreters, `None` terminates the loop (condition = false). Abstract
    /// interpreters should handle `None` by exploring both paths.
    fn loop_condition(&self, end: &Self) -> Option<bool>;
    /// Advance the induction variable by `step`.
    ///
    /// Returns `None` on arithmetic overflow/underflow.
    fn loop_step(&self, step: &Self) -> Option<Self>
    where
        Self: Sized;
}

impl ForLoopValue for i64 {
    fn loop_condition(&self, end: &i64) -> Option<bool> {
        Some(*self < *end)
    }

    fn loop_step(&self, step: &i64) -> Option<i64> {
        self.checked_add(*step)
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
        assert_eq!(0i64.loop_step(&1), Some(1));
        assert_eq!(5i64.loop_step(&3), Some(8));
    }

    #[test]
    fn loop_step_negative() {
        assert_eq!(10i64.loop_step(&-1), Some(9));
        assert_eq!(0i64.loop_step(&-5), Some(-5));
    }

    #[test]
    fn loop_step_zero() {
        assert_eq!(42i64.loop_step(&0), Some(42));
    }

    #[test]
    fn loop_step_from_negative() {
        assert_eq!((-10i64).loop_step(&3), Some(-7));
    }

    #[test]
    fn loop_step_overflow_returns_none() {
        assert_eq!(i64::MAX.loop_step(&1), None);
        assert_eq!((i64::MAX - 1).loop_step(&2), None);
    }

    #[test]
    fn loop_step_underflow_returns_none() {
        assert_eq!(i64::MIN.loop_step(&-1), None);
        assert_eq!((i64::MIN + 1).loop_step(&-2), None);
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
            iv = iv.loop_step(&step).unwrap();
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
            iv = iv.loop_step(&step).unwrap();
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
            current = current.loop_step(&1).unwrap();
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
            iv = iv.loop_step(&step).unwrap();
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

impl<'ir, I, T> Interpretable<'ir, I> for If<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + BranchCondition + ProductValue,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let cond = interp.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Ok(Continuation::Fork(smallvec![
                    (self.then_body, smallvec![]),
                    (self.else_body, smallvec![]),
                ]));
            }
        };
        let stage = interp.active_stage_info::<L>();
        interp.bind_block_args(stage, block, &[])?;
        let control = interp.eval_block(stage, block)?;
        match control {
            Continuation::Yield(v) => {
                interp.write_statement_results(&self.results, v)?;
                Ok(Continuation::Continue)
            }
            other => Ok(other),
        }
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for For<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ForLoopValue + ProductValue,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let mut iv = interp.read(self.start)?;
        let end = interp.read(self.end)?;
        let step = interp.read(self.step)?;

        // Initialize loop-carried state: pack init_args into a product (single V).
        let init_values: Vec<I::Value> = self
            .init_args
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let mut carried = ProductValue::new_product(init_values);

        let stage = interp.active_stage_info::<L>();
        while iv.loop_condition(&end) == Some(true) {
            // Bind induction variable as the first block argument, followed by carried values.
            let mut block_args = Vec::with_capacity(1 + self.init_args.len());
            block_args.push(iv.clone());
            if let Some(product) = carried.as_product() {
                block_args.extend(product.iter().cloned());
            } else if !self.init_args.is_empty() {
                // Single carried value (not a product)
                block_args.push(carried.clone());
            }
            interp.bind_block_args(stage, self.body, &block_args)?;

            let control = interp.eval_block(stage, self.body)?;
            match control {
                Continuation::Yield(v) => {
                    // Yielded value is the next loop-carried state (product or single).
                    carried = v;
                }
                other => return Ok(other),
            }
            iv = iv.loop_step(&step).ok_or_else(|| {
                I::Error::from(InterpreterError::Custom(
                    "scf.for: induction variable overflow during loop step".into(),
                ))
            })?;
        }

        // Write final loop-carried state to results via auto-destructuring.
        interp.write_statement_results(&self.results, carried)?;

        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Yield<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ProductValue,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let values: Vec<I::Value> = self
            .values
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = <I::Value as ProductValue>::new_product(values);
        Ok(Continuation::Yield(product))
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for StructuredControlFlow<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + BranchCondition + ForLoopValue + ProductValue,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            StructuredControlFlow::If(op) => op.interpret::<L>(interp),
            StructuredControlFlow::For(op) => op.interpret::<L>(interp),
            StructuredControlFlow::Yield(op) => op.interpret::<L>(interp),
        }
    }
}
