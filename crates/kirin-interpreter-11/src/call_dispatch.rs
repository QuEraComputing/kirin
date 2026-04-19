use kirin_ir::{CompileStage, Pipeline, SpecializedFunction, StageMeta};

use crate::error::InterpreterError;

/// Cursor-creation hook for concrete cross-stage calls.
///
/// Implement on your stage container `S` to control how `ConcreteInterp`
/// creates a cursor when handling `Control::Call`. Enables:
///
/// - **Single-stage**: always create `BlockCursor<V, L>` (via `Lift<C>` identity).
/// - **Multi-stage**: match on `stage_id` and create the appropriate cursor variant.
///
/// The returned `C` must be a variant of the interpreter's cursor coproduct.
/// The `stage_id` comes from the `Control::Call.stage` field emitted by dialect ops
/// (e.g., `eval_call_for_dialect`), which records where the callee lives.
pub trait CallDispatch<V, C>: StageMeta + Sized {
    fn make_call_cursor(
        pipeline: &Pipeline<Self>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<C, InterpreterError>;
}
