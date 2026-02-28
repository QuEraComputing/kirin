mod call;
mod dispatch;
mod exec;
mod frame;
mod interp;
mod stage;
mod transition;

use std::collections::HashSet;
use std::marker::PhantomData;

use kirin_ir::{CompileStage, Pipeline, StageMeta, Statement};

use crate::{ConcreteContinuation, Frame, InterpreterError};

pub use dispatch::{FrameDispatchAction, PushCallFrameDynAction};
pub use stage::{InStage, WithStage};

struct StackFrameExtra<'ir, V, S, E, G>
where
    S: StageMeta,
{
    cursor: Option<Statement>,
    dispatch: DynFrameDispatch<'ir, V, S, E, G>,
}

type StackFrame<'ir, V, S, E, G> = Frame<V, StackFrameExtra<'ir, V, S, E, G>>;

struct StageDispatchTable<'ir, V, S, E, G>
where
    S: StageMeta,
{
    by_stage: Vec<Option<DynFrameDispatch<'ir, V, S, E, G>>>,
}

type DynStepFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>) -> Result<ConcreteContinuation<V>, E>;
type DynAdvanceFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>, &ConcreteContinuation<V>) -> Result<(), E>;

#[doc(hidden)]
pub struct DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    step: DynStepFn<'ir, V, S, E, G>,
    advance: DynAdvanceFn<'ir, V, S, E, G>,
}

impl<'ir, V, S, E, G> Copy for DynFrameDispatch<'ir, V, S, E, G> where S: StageMeta {}

impl<'ir, V, S, E, G> Clone for DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    fn clone(&self) -> Self {
        *self
    }
}

/// Stack-based interpreter that owns execution state and drives evaluation.
///
/// Combines value storage (frames), pipeline reference, and execution logic
/// (step/advance/run/call) in one type. Different interpreter implementations
/// (e.g. [`crate::AbstractInterpreter`]) provide different walking strategies.
///
/// # Error type
///
/// Defaults to [`InterpreterError`]. Users who need additional error variants
/// can define their own error type with `#[from] InterpreterError`:
///
/// ```ignore
/// #[derive(Debug, thiserror::Error)]
/// enum MyError {
///     #[error(transparent)]
///     Interp(#[from] InterpreterError),
///     #[error("division by zero")]
///     DivisionByZero,
/// }
///
/// let mut interp = StackInterpreter::<i64, _, MyError>::new(&pipeline, stage);
/// ```
pub struct StackInterpreter<'ir, V, S, E = InterpreterError, G = ()>
where
    S: StageMeta,
{
    call_stack: Vec<StackFrame<'ir, V, S, E, G>>,
    dispatch_table: StageDispatchTable<'ir, V, S, E, G>,
    global: G,
    pipeline: &'ir Pipeline<S>,
    root_stage: CompileStage,
    breakpoints: HashSet<Statement>,
    fuel: Option<u64>,
    max_depth: Option<usize>,
    _error: PhantomData<E>,
}
