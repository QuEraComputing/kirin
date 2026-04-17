use kirin::prelude::{CompileTimeValue, GetInfo, SpecializedFunction, StageInfo};
use kirin_interpreter::ProductValue;
use kirin_interpreter_4::effect::{CallPayload, CursorEffect, ReturnEffect};
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{
    Interpretable, Interpreter, Machine, PipelineAccess, ValueStore,
};

use crate::{Bind, Call, FunctionBody, Lifted, Return};

// ---------------------------------------------------------------------------
// FunctionBody — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for FunctionBody<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = CursorEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        _interp: &mut I,
    ) -> Result<CursorEffect<<I as ValueStore>::Value>, InterpreterError> {
        Err(InterpreterError::UnhandledEffect(
            "function bodies are structural and should not be stepped directly".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Bind — not yet supported
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for Bind<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = CursorEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        _interp: &mut I,
    ) -> Result<CursorEffect<<I as ValueStore>::Value>, InterpreterError> {
        Err(InterpreterError::UnhandledEffect(
            "bind is not yet supported in interpreter4".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Return
// ---------------------------------------------------------------------------

impl<I, T> Interpretable<I> for Return<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone + ProductValue,
    ReturnEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = ReturnEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<ReturnEffect<<I as ValueStore>::Value>, InterpreterError> {
        let values: Vec<<I as ValueStore>::Value> = self
            .values
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = <<I as ValueStore>::Value as ProductValue>::new_product(values);
        Ok(ReturnEffect(product))
    }
}

// ---------------------------------------------------------------------------
// Call
// ---------------------------------------------------------------------------

/// Resolve a symbol to a [`SpecializedFunction`] via the pipeline.
fn resolve_call<I, L>(
    interp: &I,
    target: kirin::prelude::Symbol,
) -> Result<SpecializedFunction, InterpreterError>
where
    I: PipelineAccess<StageInfo = StageInfo<L>>,
    L: kirin::prelude::Dialect,
{
    let stage_id = interp.current_stage();
    let stage = interp
        .pipeline()
        .stage(stage_id)
        .ok_or(InterpreterError::MissingEntry)?;

    let target_name = stage
        .symbol_table()
        .resolve(target)
        .cloned()
        .unwrap_or_else(|| format!("{target:?}"));

    let function = interp
        .pipeline()
        .resolve_function(stage, target)
        .ok_or_else(|| {
            InterpreterError::UnhandledEffect(format!("unknown function: {target_name}"))
        })?;

    let staged_function = interp
        .pipeline()
        .function_info(function)
        .and_then(|info| info.staged_function(stage_id))
        .ok_or_else(|| {
            InterpreterError::UnhandledEffect(format!("missing staged function: {target_name}"))
        })?;

    let callee = staged_function
        .get_info(stage)
        .ok_or_else(|| {
            InterpreterError::UnhandledEffect(format!("missing specialization info: {target_name}"))
        })?
        .unique_live_specialization()
        .map_err(|_| {
            InterpreterError::UnhandledEffect(format!(
                "no unique specialization for: {target_name}"
            ))
        })?;

    Ok(callee)
}

impl<I, T, L> Interpretable<I> for Call<T>
where
    I: Interpreter + Machine<Error = InterpreterError> + PipelineAccess<StageInfo = StageInfo<L>>,
    <I as ValueStore>::Value: Clone,
    CallPayload<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
    L: kirin::prelude::Dialect,
{
    type Effect = CallPayload<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<CallPayload<<I as ValueStore>::Value>, InterpreterError> {
        let args = interp.read_many(self.args())?;
        let callee_stage = interp.current_stage();
        let callee = resolve_call(interp, self.target())?;
        Ok(CallPayload {
            callee,
            callee_stage,
            args,
            results: self.results().to_vec(),
        })
    }
}

// ---------------------------------------------------------------------------
// Lifted — delegates to inner types
// ---------------------------------------------------------------------------

impl<I, T, L> Interpretable<I> for Lifted<T>
where
    I: Interpreter + Machine<Error = InterpreterError> + PipelineAccess<StageInfo = StageInfo<L>>,
    <I as ValueStore>::Value: Clone + ProductValue,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    ReturnEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    CallPayload<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
    L: kirin::prelude::Dialect,
{
    type Effect = <I as Machine>::Effect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut I) -> Result<<I as Machine>::Effect, InterpreterError> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(interp).map(LiftInto::lift_into),
            Lifted::Bind(op) => op.interpret(interp).map(LiftInto::lift_into),
            Lifted::Call(op) => op.interpret(interp).map(LiftInto::lift_into),
            Lifted::Return(op) => op.interpret(interp).map(LiftInto::lift_into),
        }
    }
}
