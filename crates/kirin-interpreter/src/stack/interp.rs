use std::collections::HashSet;

use kirin_ir::{CompileStage, Pipeline, StageMeta, Statement, SupportsStageDispatch};

use super::{DynFrameDispatch, FrameDispatchAction, StackInterpreter};
use crate::InterpreterError;

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> StackInterpreter<'ir, V, S, E, ()>
where
    V: Clone + 'ir,
    S: StageMeta,
    E: From<InterpreterError> + 'ir,
    S: SupportsStageDispatch<
            FrameDispatchAction<'ir, V, S, E, ()>,
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
            FrameDispatchAction<'ir, V, S, E, G>,
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
            call_stack: Vec::new(),
            dispatch_table,
            global,
            pipeline,
            root_stage: stage,
            breakpoints: HashSet::default(),
            fuel: None,
            max_depth: None,
            _error: std::marker::PhantomData,
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
        self.max_depth = Some(depth);
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
    pub fn set_breakpoints(&mut self, stmts: HashSet<Statement>) {
        self.breakpoints = stmts;
    }

    /// Clear all configured breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }
}
