use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageAction, StageInfo, StageMeta, SupportsStageDispatch, Symbol,
};

use crate::cursor::{BlockCursor, Boxed, Execute};
use crate::effect::ControlFlow;
use crate::env::Env;
use crate::error::InterpreterError;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;

// ---------------------------------------------------------------------------
// ConcreteDomain — extended domain interface for cursor execution
// ---------------------------------------------------------------------------

/// Extended [`Env`] interface for concrete execution.
///
/// Cursors (types implementing [`Execute`]) use `ConcreteDomain` to:
/// - Look up stage info by stage ID and dialect type.
/// - Create sub-cursors for blocks (used by SCF-style cursors).
/// - Resolve function symbols to [`SpecializedFunction`].
/// - Consume pending yield values produced by nested executions.
///
/// The `Execute<D>` impls require `D: ConcreteDomain` plus
/// `D: Env<Effect = ControlFlow<D::Value, Boxed<D>>>` at the impl site.
/// Splitting these avoids a cycle in trait super-predicate resolution.
pub trait ConcreteDomain: Env + Sized {
    type StageContainer: StageMeta;

    /// Look up the [`StageInfo<L>`] for `stage_id`, if this domain knows about `L`.
    fn stage_info_for<L: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<L>>
    where
        Self::StageContainer: HasStageInfo<L>;

    /// Create a [`Boxed`] cursor for `block` at the given `stage_id` with `args`.
    fn make_block_cursor(
        &mut self,
        block: Block,
        stage_id: CompileStage,
        args: Vec<Self::Value>,
    ) -> Result<Boxed<Self>, Self::Error>;

    /// Resolve a stage-local symbol to a [`SpecializedFunction`] at `stage_id`.
    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, Self::Error>;

    /// Take and clear the pending yield value, if any.
    fn take_pending_yield(&mut self) -> Option<Self::Value>;
}

// ===========================================================================
// ConcreteInterp — single-dialect concrete interpreter
// ===========================================================================

/// Single-dialect concrete interpreter.
///
/// Executes a program in dialect `L` at one compilation stage. The cursor stack
/// contains `BlockCursor<V, L>` entries boxed as `Boxed<Self>`.
pub struct ConcreteInterp<'ir, L: Dialect, V: Clone + 'static> {
    pipeline: &'ir Pipeline<StageInfo<L>>,
    stage_id: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<Boxed<Self>>,
    pending_yield: Option<V>,
}

// -- Env --------------------------------------------------------------------

impl<'ir, L, V> Env for ConcreteInterp<'ir, L, V>
where
    L: Dialect,
    V: Clone + 'static,
{
    type Value = V;
    type Effect = ControlFlow<V, Boxed<Self>>;
    type Error = InterpreterError;

    fn advance() -> Self::Effect {
        ControlFlow::Advance
    }

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.stage_id)
    }

    fn read(&self, value: SSAValue) -> Result<V, InterpreterError> {
        self.frames.read(value)
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), InterpreterError> {
        self.frames.write(result, value)
    }

    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), InterpreterError> {
        self.frames.write_ssa(ssa, value)
    }
}

// -- ConcreteDomain ---------------------------------------------------------

impl<'ir, L, V> ConcreteDomain for ConcreteInterp<'ir, L, V>
where
    L: Dialect,
    V: Clone + 'static,
    BlockCursor<V, L>: Execute<Self> + 'static,
{
    type StageContainer = StageInfo<L>;

    fn stage_info_for<LD: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<LD>>
    where
        StageInfo<L>: HasStageInfo<LD>,
    {
        self.pipeline.stage(stage_id)?.try_stage_info()
    }

    fn make_block_cursor(
        &mut self,
        block: Block,
        _stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<Boxed<Self>, InterpreterError> {
        let cursor = BlockCursor::<V, L>::new(block, self.stage_id, args);
        Ok(Boxed(Box::new(cursor)))
    }

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError> {
        let stage = self
            .pipeline
            .stage(stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        let function = self
            .pipeline
            .resolve_function(stage, target)
            .ok_or(InterpreterError::MissingEntry)?;
        let staged_function = self
            .pipeline
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or(InterpreterError::MissingEntry)?;
        staged_function
            .get_info(stage)
            .ok_or(InterpreterError::MissingEntry)?
            .unique_live_specialization()
            .map_err(|_| InterpreterError::UnhandledEffect("ambiguous specialization".into()))
    }

    fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

// -- Constructor ------------------------------------------------------------

impl<'ir, L: Dialect, V: Clone + 'static> ConcreteInterp<'ir, L, V> {
    pub fn new(pipeline: &'ir Pipeline<StageInfo<L>>, stage_id: CompileStage) -> Self {
        Self {
            pipeline,
            stage_id,
            frames: FrameStack::new(),
            cursors: Vec::new(),
            pending_yield: None,
        }
    }

    /// Take a pending yield value, if any.
    pub fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

// -- enter_function ---------------------------------------------------------

impl<'ir, L, V> ConcreteInterp<'ir, L, V>
where
    L: Dialect,
    V: Clone + 'static,
    BlockCursor<V, L>: Execute<Self> + 'static,
{
    /// Enter a function at dialect `LD`, pushing a frame and a [`BlockCursor`]
    /// positioned at `entry_block` with the given arguments.
    pub fn enter_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        StageInfo<L>: HasStageInfo<LD>,
        BlockCursor<V, LD>: Execute<Self> + 'static,
    {
        let frame = Frame::new(callee, self.stage_id, vec![]);
        self.frames.push(frame)?;
        let cursor = BlockCursor::<V, LD>::new(entry_block, self.stage_id, args.to_vec());
        self.cursors.push(Boxed(Box::new(cursor)));
        Ok(())
    }
}

// -- Driver loop (step / run) -----------------------------------------------

impl<'ir, L, V> ConcreteInterp<'ir, L, V>
where
    L: Dialect,
    V: Clone + 'static,
    BlockCursor<V, L>: Execute<Self> + 'static,
{
    /// Execute one step of the driver loop.
    ///
    /// Returns `true` if a step was executed, `false` if the cursor stack is
    /// empty (execution complete).
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut entry) = self.cursors.pop() else {
            return Ok(false);
        };

        let effect = entry.execute(self)?;

        match effect {
            ControlFlow::Advance => {
                self.cursors.push(entry);
            }
            ControlFlow::Jump(_, _) => {
                self.cursors.push(entry);
            }
            ControlFlow::Push(new_entry) => {
                self.cursors.push(entry);
                self.cursors.push(new_entry);
            }
            ControlFlow::Pop => {
                // Cursor self-removes. Drop entry, no side effects.
            }
            ControlFlow::Yield(v) => {
                self.pending_yield = Some(v);
                // Do NOT push entry back — yielded, cursor is done.
            }
            ControlFlow::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    // Top-level return: treat as yield.
                    self.pending_yield = Some(v);
                } else {
                    for result in &caller_results {
                        self.frames.write(*result, v.clone())?;
                    }
                }
                // Do NOT push entry back — the cursor is done.
            }
            ControlFlow::Call {
                callee,
                stage,
                args,
                results,
            } => {
                self.cursors.push(entry);
                self.push_call_frame(callee, stage, args, results)?;
            }
        }

        Ok(true)
    }

    /// Run until the cursor stack is empty or a yield is produced.
    pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
        while self.step()? {}
        Ok(self.pending_yield.take())
    }

    fn push_call_frame(
        &mut self,
        callee: SpecializedFunction,
        _callee_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<(), InterpreterError> {
        let stage = self
            .pipeline
            .stage(self.stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        let spec_info = callee
            .get_info(stage)
            .ok_or(InterpreterError::MissingEntry)?;
        let body_stmt = *spec_info.body();
        let definition = body_stmt.definition(stage);
        let entry_region = definition
            .regions()
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        let entry_block = entry_region
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)?;

        let frame = Frame::new(callee, self.stage_id, results);
        self.frames.push(frame)?;

        let cursor = BlockCursor::<V, L>::new(entry_block, self.stage_id, args);
        self.cursors.push(Boxed(Box::new(cursor)));
        Ok(())
    }
}

// ===========================================================================
// MultiStageInterp — multi-dialect concrete interpreter
// ===========================================================================

/// Multi-dialect concrete interpreter.
///
/// Executes programs that span multiple dialects/stages. Each `BlockCursor<V, L>`
/// is erased to [`Boxed`] so the cursor stack is heterogeneous. The `push_call_frame`
/// method uses `StageDispatch` (via [`PushCallAction`]) to create the right
/// `BlockCursor<V, L>` for the callee's stage.
///
/// The dispatch closures are stored in the struct (initialized in `new`) so that
/// `impl ConcreteDomain` carries no `SupportsStageDispatch` bounds. This breaks the
/// otherwise-cyclic trait dependency:
///   `ConcreteDomain for MultiStageInterp` → `PushBlockAction: StageAction<S, L>`
///   → `BlockCursor<V, L>: Execute<MultiStageInterp>` → `L: Interpretable<MultiStageInterp>`
///   → `MultiStageInterp: ConcreteDomain`.
type MakeCursorFn<'ir, S, V> = Box<
    dyn Fn(
            Block,
            CompileStage,
            Vec<V>,
        ) -> Result<Boxed<MultiStageInterp<'ir, S, V>>, InterpreterError>
        + 'ir,
>;

type PushCallFn<'ir, S, V> = Box<
    dyn Fn(
            SpecializedFunction,
            CompileStage,
            Vec<V>,
        ) -> Result<Boxed<MultiStageInterp<'ir, S, V>>, InterpreterError>
        + 'ir,
>;

type ResolveFn<'ir> =
    Box<dyn Fn(Symbol, CompileStage) -> Result<SpecializedFunction, InterpreterError> + 'ir>;

pub struct MultiStageInterp<'ir, S: StageMeta, V: Clone + 'static> {
    pipeline: &'ir Pipeline<S>,
    root_stage: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<Boxed<Self>>,
    pending_yield: Option<V>,
    make_cursor_fn: MakeCursorFn<'ir, S, V>,
    push_call_fn: PushCallFn<'ir, S, V>,
    resolve_fn: ResolveFn<'ir>,
}

// -- Env --------------------------------------------------------------------

impl<'ir, S, V> Env for MultiStageInterp<'ir, S, V>
where
    S: StageMeta,
    V: Clone + 'static,
{
    type Value = V;
    type Effect = ControlFlow<V, Boxed<Self>>;
    type Error = InterpreterError;

    fn advance() -> Self::Effect {
        ControlFlow::Advance
    }

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.root_stage)
    }

    fn read(&self, value: SSAValue) -> Result<V, InterpreterError> {
        self.frames.read(value)
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), InterpreterError> {
        self.frames.write(result, value)
    }

    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), InterpreterError> {
        self.frames.write_ssa(ssa, value)
    }
}

// -- ConcreteDomain ---------------------------------------------------------

impl<'ir, S, V> ConcreteDomain for MultiStageInterp<'ir, S, V>
where
    S: StageMeta,
    V: Clone + 'static,
{
    type StageContainer = S;

    fn stage_info_for<L: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<L>>
    where
        S: HasStageInfo<L>,
    {
        self.pipeline.stage(stage_id)?.try_stage_info()
    }

    fn make_block_cursor(
        &mut self,
        block: Block,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<Boxed<Self>, InterpreterError> {
        (self.make_cursor_fn)(block, stage_id, args)
    }

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError> {
        (self.resolve_fn)(target, stage_id)
    }

    fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

// -- Constructor ------------------------------------------------------------

impl<'ir, S, V> MultiStageInterp<'ir, S, V>
where
    S: StageMeta
        + SupportsStageDispatch<PushBlockAction<'ir, S, V>, (), InterpreterError>
        + SupportsStageDispatch<PushCallAction<'ir, S, V>, (), InterpreterError>
        + SupportsStageDispatch<ResolveFunctionAction<'ir, S>, (), InterpreterError>,
    V: Clone + 'static,
{
    pub fn new(pipeline: &'ir Pipeline<S>, root_stage: CompileStage) -> Self {
        Self {
            pipeline,
            root_stage,
            frames: FrameStack::new(),
            cursors: Vec::new(),
            pending_yield: None,
            make_cursor_fn: Box::new(move |block, stage_id, args| {
                let stage_container = pipeline
                    .stage(stage_id)
                    .ok_or(InterpreterError::MissingEntry)?;
                let mut action = PushBlockAction {
                    block,
                    stage_id,
                    args,
                    result: None,
                };
                S::dispatch_stage_action(stage_container, stage_id, &mut action)?
                    .ok_or(InterpreterError::MissingEntry)?;
                action.result.ok_or(InterpreterError::MissingEntry)
            }),
            push_call_fn: Box::new(move |callee, stage_id, args| {
                let stage_container = pipeline
                    .stage(stage_id)
                    .ok_or(InterpreterError::MissingEntry)?;
                let mut action = PushCallAction {
                    callee,
                    args,
                    result: None,
                };
                S::dispatch_stage_action(stage_container, stage_id, &mut action)?
                    .ok_or(InterpreterError::MissingEntry)?;
                action.result.ok_or(InterpreterError::MissingEntry)
            }),
            resolve_fn: Box::new(move |target, stage_id| {
                let stage_container = pipeline
                    .stage(stage_id)
                    .ok_or(InterpreterError::MissingEntry)?;
                let mut action = ResolveFunctionAction {
                    pipeline,
                    target,
                    result: None,
                };
                S::dispatch_stage_action(stage_container, stage_id, &mut action)?
                    .ok_or(InterpreterError::MissingEntry)?;
                action.result.ok_or(InterpreterError::MissingEntry)
            }),
        }
    }

    /// Take a pending yield value, if any.
    pub fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

// -- enter_function ---------------------------------------------------------

impl<'ir, S, V> MultiStageInterp<'ir, S, V>
where
    S: StageMeta,
    V: Clone + 'static,
{
    /// Enter a function at dialect `L`, pushing a frame and a [`BlockCursor`]
    /// positioned at `entry_block` with the given arguments.
    pub fn enter_function<L: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        S: HasStageInfo<L>,
        BlockCursor<V, L>: Execute<Self> + 'static,
    {
        let cursor = BlockCursor::<V, L>::new(entry_block, self.root_stage, args.to_vec());
        let frame = Frame::new(callee, self.root_stage, vec![]);
        self.frames.push(frame)?;
        self.cursors.push(Boxed(Box::new(cursor)));
        Ok(())
    }
}

// -- Driver loop (step / run) -----------------------------------------------

impl<'ir, S, V> MultiStageInterp<'ir, S, V>
where
    S: StageMeta,
    V: Clone + 'static,
{
    /// Execute one step of the driver loop.
    ///
    /// Returns `true` if a step was executed, `false` if the cursor stack is
    /// empty (execution complete).
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut entry) = self.cursors.pop() else {
            return Ok(false);
        };

        let effect = entry.execute(self)?;

        match effect {
            ControlFlow::Advance => {
                self.cursors.push(entry);
            }
            ControlFlow::Jump(_, _) => {
                self.cursors.push(entry);
            }
            ControlFlow::Push(new_entry) => {
                self.cursors.push(entry);
                self.cursors.push(new_entry);
            }
            ControlFlow::Pop => {
                // Cursor self-removes. Drop entry, no side effects.
            }
            ControlFlow::Yield(v) => {
                self.pending_yield = Some(v);
                // Do NOT push entry back — yielded, cursor is done.
            }
            ControlFlow::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    // Top-level return: treat as yield.
                    self.pending_yield = Some(v);
                } else {
                    for result in &caller_results {
                        self.frames.write(*result, v.clone())?;
                    }
                }
                // Do NOT push entry back — the cursor is done.
            }
            ControlFlow::Call {
                callee,
                stage,
                args,
                results,
            } => {
                self.cursors.push(entry);
                self.push_call_frame(callee, stage, args, results)?;
            }
        }

        Ok(true)
    }

    /// Run until the cursor stack is empty or a yield is produced.
    pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
        while self.step()? {}
        Ok(self.pending_yield.take())
    }

    fn push_call_frame(
        &mut self,
        callee: SpecializedFunction,
        callee_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<(), InterpreterError> {
        let cursor = (self.push_call_fn)(callee, callee_stage, args)?;
        let frame = Frame::new(callee, callee_stage, results);
        self.frames.push(frame)?;
        self.cursors.push(cursor);
        Ok(())
    }
}

// ===========================================================================
// StageAction types for MultiStageInterp dispatch
// ===========================================================================

/// [`StageAction`] that creates a [`Boxed`] cursor for an existing block.
///
/// Used by `MultiStageInterp::make_block_cursor` to dispatch to the right
/// dialect `L` for a given `stage_id`.
pub struct PushBlockAction<'ir, S: StageMeta, V: Clone + 'static> {
    pub block: Block,
    pub stage_id: CompileStage,
    pub args: Vec<V>,
    /// Populated by `run`; consumed by the caller.
    pub result: Option<Boxed<MultiStageInterp<'ir, S, V>>>,
}

impl<'ir, S, L, V> StageAction<S, L> for PushBlockAction<'ir, S, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    BlockCursor<V, L>: Execute<MultiStageInterp<'ir, S, V>> + 'static,
    V: Clone + 'static,
{
    type Output = ();
    type Error = InterpreterError;

    fn run(
        &mut self,
        stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<(), InterpreterError> {
        let cursor = BlockCursor::<V, L>::new(self.block, stage_id, self.args.clone());
        self.result = Some(Boxed(Box::new(cursor)));
        Ok(())
    }
}

/// [`StageAction`] that creates a [`Boxed`] cursor for a callee function.
///
/// Used by `MultiStageInterp::push_call_frame` to dispatch to the right
/// dialect `L` for the callee's stage.
pub struct PushCallAction<'ir, S: StageMeta, V: Clone + 'static> {
    pub callee: SpecializedFunction,
    pub args: Vec<V>,
    /// Populated by `run`; consumed by the caller.
    pub result: Option<Boxed<MultiStageInterp<'ir, S, V>>>,
}

impl<'ir, S, L, V> StageAction<S, L> for PushCallAction<'ir, S, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    BlockCursor<V, L>: Execute<MultiStageInterp<'ir, S, V>> + 'static,
    V: Clone + 'static,
{
    type Output = ();
    type Error = InterpreterError;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &StageInfo<L>,
    ) -> Result<(), InterpreterError> {
        let spec_info = self
            .callee
            .get_info(stage)
            .ok_or(InterpreterError::MissingEntry)?;
        let body_stmt = *spec_info.body();
        let definition = body_stmt.definition(stage);
        let entry_region = definition
            .regions()
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        let entry_block = entry_region
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        let cursor = BlockCursor::<V, L>::new(entry_block, stage_id, self.args.clone());
        self.result = Some(Boxed(Box::new(cursor)));
        Ok(())
    }
}

/// [`StageAction`] that resolves a stage-local symbol to a [`SpecializedFunction`].
///
/// Used by `MultiStageInterp::resolve_function` to dispatch to the right
/// dialect `L` for a given `stage_id`.
pub struct ResolveFunctionAction<'ir, S: StageMeta> {
    pub pipeline: &'ir Pipeline<S>,
    pub target: Symbol,
    /// Populated by `run`; consumed by the caller.
    pub result: Option<SpecializedFunction>,
}

impl<'ir, S, L> StageAction<S, L> for ResolveFunctionAction<'ir, S>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = ();
    type Error = InterpreterError;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &StageInfo<L>,
    ) -> Result<(), InterpreterError> {
        let function = self
            .pipeline
            .resolve_function(stage, self.target)
            .ok_or(InterpreterError::MissingEntry)?;
        let staged_function = self
            .pipeline
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or(InterpreterError::MissingEntry)?;
        let spec = staged_function
            .get_info(stage)
            .ok_or(InterpreterError::MissingEntry)?
            .unique_live_specialization()
            .map_err(|_| InterpreterError::UnhandledEffect("ambiguous specialization".into()))?;
        self.result = Some(spec);
        Ok(())
    }
}
