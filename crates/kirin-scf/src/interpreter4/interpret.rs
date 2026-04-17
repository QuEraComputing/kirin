use kirin::prelude::CompileTimeValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_4::effect::{CursorEffect, PushEffect, YieldEffect};
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{Interpretable, Interpreter, Machine, ValueStore};

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

use super::cursor::{ForCursor, IfCursor};

// ---------------------------------------------------------------------------
// If — returns PushEffect<IfCursor<V>>
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for If<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone + BranchCondition + ProductValue,
    PushEffect<IfCursor<<I as ValueStore>::Value>>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = PushEffect<IfCursor<<I as ValueStore>::Value>>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<PushEffect<IfCursor<<I as ValueStore>::Value>>, InterpreterError> {
        let cond = interp.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(InterpreterError::UnhandledEffect(
                    "scf.if: nondeterministic conditions not supported in interpreter4".into(),
                ));
            }
        };

        Ok(PushEffect(IfCursor::new(
            block,
            self.results.clone(),
            interp.current_stage(),
        )))
    }
}

// ---------------------------------------------------------------------------
// For — returns PushEffect<ForCursor<V>>
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for For<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone + ForLoopValue + ProductValue,
    PushEffect<ForCursor<<I as ValueStore>::Value>>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = PushEffect<ForCursor<<I as ValueStore>::Value>>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<PushEffect<ForCursor<<I as ValueStore>::Value>>, InterpreterError> {
        let iv = interp.read(self.start)?;
        let end = interp.read(self.end)?;
        let step = interp.read(self.step)?;

        let init_values: Vec<<I as ValueStore>::Value> = self
            .init_args
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let init_arg_count = init_values.len();
        let carried = <<I as ValueStore>::Value as ProductValue>::new_product(init_values);

        Ok(PushEffect(ForCursor::new(
            iv,
            end,
            step,
            carried,
            self.body,
            init_arg_count,
            self.results.clone(),
            interp.current_stage(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Yield — returns YieldEffect<V>
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for Yield<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone + ProductValue,
    YieldEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = YieldEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<YieldEffect<<I as ValueStore>::Value>, InterpreterError> {
        let values: Vec<<I as ValueStore>::Value> = self
            .values
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = <<I as ValueStore>::Value as ProductValue>::new_product(values);
        Ok(YieldEffect(product))
    }
}

// ---------------------------------------------------------------------------
// StructuredControlFlow — wrapping enum
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for StructuredControlFlow<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone + BranchCondition + ForLoopValue + ProductValue,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    PushEffect<IfCursor<<I as ValueStore>::Value>>: LiftInto<<I as Machine>::Effect>,
    PushEffect<ForCursor<<I as ValueStore>::Value>>: LiftInto<<I as Machine>::Effect>,
    YieldEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = <I as Machine>::Effect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut I) -> Result<<I as Machine>::Effect, InterpreterError> {
        match self {
            Self::If(op) => op.interpret(interp).map(LiftInto::lift_into),
            Self::For(op) => op.interpret(interp).map(LiftInto::lift_into),
            Self::Yield(op) => op.interpret(interp).map(LiftInto::lift_into),
        }
    }
}
