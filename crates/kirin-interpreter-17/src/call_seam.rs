use kirin_interpreter::AbstractValue;
use kirin_ir::{CompileStage, Dialect, HasStageInfo, ResultValue, StageMeta, Symbol};

use crate::abstract_call_dispatch::AbstractCallDispatch;
use crate::abstract_interp::AbstractInterp;
use crate::algebra::SingleStageCursorFor;
use crate::concrete::ConcreteInterp;
use crate::control::{Control, CursorExt};
use crate::env::Env;
use crate::error::InterpreterError;

/// Extension point for dialect-aware call dispatch.
///
/// Parameterised by `L: Dialect` so that dialect-local types (e.g. `HighLevel`,
/// `LowLevel`) appear as **uncovered** types in the trait parameter, satisfying
/// the orphan rule for multi-stage interpreters defined outside this crate.
///
/// The method receives raw call operands rather than `&Call<T>` to avoid a
/// circular dependency between kirin-interpreter-17 and kirin-function.
pub trait CallSeam<L: Dialect>: Env {
    fn eval_call(
        &mut self,
        target: Symbol,
        stage: CompileStage,
        args: Vec<Self::Value>,
        results: Vec<ResultValue>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
}

// ---------------------------------------------------------------------------
// Blanket impl for ConcreteInterp — single-stage case.
//
// `SingleStageCursorFor<L>` prevents this from applying to multi-stage
// cursors; those provide their own impl in the user's crate using a local
// dialect type as the uncovered trait parameter.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> CallSeam<L> for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: SingleStageCursorFor<L>,
    Self::Error: From<InterpreterError>,
{
    fn eval_call(
        &mut self,
        target: Symbol,
        stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let callee = self.resolve_function_for::<L>(target, stage)?;
        Ok(Control::Call {
            callee,
            stage,
            args,
            results,
        })
    }
}

// ---------------------------------------------------------------------------
// Blanket impl for AbstractInterp — single-stage case.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> CallSeam<L> for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue,
    C: SingleStageCursorFor<L>,
    Self::Error: From<InterpreterError>,
{
    fn eval_call(
        &mut self,
        target: Symbol,
        stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let callee = self.resolve_function_for::<L>(target, stage)?;
        Ok(Control::Call {
            callee,
            stage,
            args,
            results,
        })
    }
}
