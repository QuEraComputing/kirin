use kirin_interpreter::AbstractValue;
use kirin_ir::{CompileStage, Dialect, HasStageInfo, ResultValue, StageMeta, Symbol};

use crate::abstract_call_dispatch::AbstractCallDispatch;
use crate::abstract_interp::AbstractInterp;
use crate::algebra::SingleStageCursorFor;
use crate::concrete::ConcreteInterp;
use crate::control::{Control, CursorExt};
use crate::env::Env;
use crate::error::InterpreterError;

/// Extension point for call dispatch — without a dialect-type parameter.
///
/// Unlike `CallSeam<L>` (iteration 17), `Dispatch` is not parameterized by
/// the calling dialect.  Multi-stage interpreters write a single impl that
/// uses the runtime `caller_stage` to choose the routing strategy, eliminating
/// the need for one impl per dialect.
///
/// Single-stage concrete and abstract interpreters receive a blanket impl
/// (gated by `SingleStageCursorFor<L>`) — no user code required.
pub trait Dispatch: Env {
    fn dispatch_call(
        &mut self,
        target: Symbol,
        caller_stage: CompileStage,
        args: Vec<Self::Value>,
        results: Vec<ResultValue>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
}

// ---------------------------------------------------------------------------
// Blanket impl for ConcreteInterp — single-stage case.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> Dispatch for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: SingleStageCursorFor<L>,
    InterpreterError: From<InterpreterError>,
{
    fn dispatch_call(
        &mut self,
        target: Symbol,
        caller_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let callee = self.resolve_function_for::<L>(target, caller_stage)?;
        Ok(Control::Call {
            callee,
            stage: caller_stage,
            args,
            results,
        })
    }
}

// ---------------------------------------------------------------------------
// Blanket impl for AbstractInterp — single-stage case.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> Dispatch for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue,
    C: SingleStageCursorFor<L>,
    InterpreterError: From<InterpreterError>,
{
    fn dispatch_call(
        &mut self,
        target: Symbol,
        caller_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let callee = self.resolve_function_for::<L>(target, caller_stage)?;
        Ok(Control::Call {
            callee,
            stage: caller_stage,
            args,
            results,
        })
    }
}
