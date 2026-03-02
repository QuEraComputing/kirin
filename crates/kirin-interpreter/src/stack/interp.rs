use std::marker::PhantomData;

use kirin_ir::{CompileStage, Pipeline, StageMeta, Statement, SupportsStageDispatch};
use rustc_hash::FxHashSet;

use super::dispatch::DynFrameDispatch;
use crate::dispatch::DispatchCache;
use crate::{Frame, FrameStack, InterpreterError};

pub(super) struct StackFrameExtra<'ir, V, S, E, G>
where
    S: StageMeta,
{
    pub(super) cursor: Option<Statement>,
    pub(super) dispatch: DynFrameDispatch<'ir, V, S, E, G>,
}

pub(super) type StackFrame<'ir, V, S, E, G> = Frame<V, StackFrameExtra<'ir, V, S, E, G>>;

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
    pub(super) frames: FrameStack<V, StackFrameExtra<'ir, V, S, E, G>>,
    pub(super) dispatch_table: DispatchCache<DynFrameDispatch<'ir, V, S, E, G>>,
    pub(super) global: G,
    pub(super) pipeline: &'ir Pipeline<S>,
    pub(super) root_stage: CompileStage,
    pub(super) breakpoints: FxHashSet<Statement>,
    pub(super) fuel: Option<u64>,
    pub(super) _error: PhantomData<E>,
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> StackInterpreter<'ir, V, S, E, ()>
where
    V: Clone + 'ir,
    S: StageMeta,
    E: From<InterpreterError> + 'ir,
    S: SupportsStageDispatch<
            super::FrameDispatchAction<'ir, V, S, E, ()>,
            DynFrameDispatch<'ir, V, S, E, ()>,
            E,
        >,
{
    /// Create a stack interpreter with unit global state.
    ///
    /// The interpreter is rooted at `stage` when no call frame is active.
    /// Per-stage dynamic dispatch is precomputed from `pipeline`.
    pub fn new(pipeline: &'ir Pipeline<S>, stage: CompileStage) -> Self {
        Self::new_with_global(pipeline, stage, ())
    }
}

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    S: StageMeta,
    E: From<InterpreterError> + 'ir,
    S: SupportsStageDispatch<
            super::FrameDispatchAction<'ir, V, S, E, G>,
            DynFrameDispatch<'ir, V, S, E, G>,
            E,
        >,
    G: 'ir,
{
    /// Create a stack interpreter with explicit global state.
    ///
    /// The interpreter is rooted at `stage` when no call frame is active.
    /// Per-stage dynamic dispatch is precomputed from `pipeline`.
    pub fn new_with_global(pipeline: &'ir Pipeline<S>, stage: CompileStage, global: G) -> Self {
        let dispatch_table = Self::build_dispatch_table(pipeline);
        Self {
            frames: FrameStack::new(),
            dispatch_table,
            global,
            pipeline,
            root_stage: stage,
            breakpoints: FxHashSet::default(),
            fuel: None,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    S: StageMeta,
{
    /// Set an instruction budget for execution.
    ///
    /// Each executed statement consumes one unit. Exceeding the budget
    /// returns [`InterpreterError::FuelExhausted`].
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    /// Set the maximum call-frame depth.
    ///
    /// Pushing beyond this limit returns [`InterpreterError::MaxDepthExceeded`].
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.frames = self.frames.with_max_depth(depth);
        self
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    S: StageMeta,
{
    /// Borrow immutable interpreter-global state.
    pub fn global(&self) -> &G {
        &self.global
    }

    /// Borrow mutable interpreter-global state.
    pub fn global_mut(&mut self) -> &mut G {
        &mut self.global
    }

    /// Replace the current breakpoint set.
    ///
    /// Breakpoints are only observed by `run_until_break*` entrypoints.
    pub fn set_breakpoints(&mut self, stmts: FxHashSet<Statement>) {
        self.breakpoints = stmts;
    }

    /// Clear all configured breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }
}
