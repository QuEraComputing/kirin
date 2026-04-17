use crate::cursor::BlockCursor;
use crate::effect::{CallPayload, CursorEffect, PopEffect, PushEffect, ReturnEffect, YieldEffect};
use crate::error::InterpreterError;
use crate::execute::Execute;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::lift::{Lift, ProjectMut, ProjectRef};
use crate::traits::{Machine, PipelineAccess, ValueStore};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageAction, StageInfo, StageMeta, SupportsStageDispatch,
};

// ---------------------------------------------------------------------------
// Action — the interpreter's effect algebra
// ---------------------------------------------------------------------------

/// The interpreter's own effect type.
///
/// Dialect effects are lifted into `Action` via [`Lift`]. The driver loop
/// in [`SingleStage::step`] dispatches on these variants directly.
///
/// - `V` — value type
/// - `R` — delegated (inner-machine) effect type (default `()`)
/// - `C` — cursor entry type pushed onto the global cursor stack (default `()`)
pub enum Action<V, R = (), C = ()> {
    /// Advance to the next statement in the current block.
    Advance,
    /// Jump the cursor to a different block with the given arguments.
    Jump(Block, Vec<V>),
    /// Return from the current function, writing the value to caller results.
    Return(V),
    /// Yield a value from the current inline execution.
    Yield(V),
    /// Push a new cursor entry onto the global cursor stack.
    Push(C),
    /// Remove the current cursor from the stack without side effects.
    /// Used by SCF-style cursors that handle inline execution internally.
    Pop,
    /// Call a specialized function with arguments, writing results to the given slots.
    Call(SpecializedFunction, CompileStage, Vec<V>, Vec<ResultValue>),
    /// Delegate to the inner dialect machine.
    Delegate(R),
}

// -- Lift impls: dialect effects → Action -----------------------------------

/// `()` (no effect) lifts to `Advance`.
impl<V, R, C> Lift<()> for Action<V, R, C> {
    fn lift(_: ()) -> Self {
        Action::Advance
    }
}

/// [`CursorEffect`] lifts directly into the corresponding [`Action`] variant.
impl<V, R, C> Lift<CursorEffect<V>> for Action<V, R, C> {
    fn lift(from: CursorEffect<V>) -> Self {
        match from {
            CursorEffect::Advance => Action::Advance,
            CursorEffect::Jump(block, args) => Action::Jump(block, args),
        }
    }
}

/// [`ReturnEffect`] lifts to [`Action::Return`].
impl<V, R, C> Lift<ReturnEffect<V>> for Action<V, R, C> {
    fn lift(from: ReturnEffect<V>) -> Self {
        Action::Return(from.0)
    }
}

/// [`YieldEffect`] lifts to [`Action::Yield`].
impl<V, R, C> Lift<YieldEffect<V>> for Action<V, R, C> {
    fn lift(from: YieldEffect<V>) -> Self {
        Action::Yield(from.0)
    }
}

/// [`CallPayload`] lifts to [`Action::Call`].
impl<V, R, C> Lift<CallPayload<V>> for Action<V, R, C> {
    fn lift(from: CallPayload<V>) -> Self {
        Action::Call(from.callee, from.callee_stage, from.args, from.results)
    }
}

/// [`PopEffect`] lifts to [`Action::Pop`].
impl<V, R, C> Lift<PopEffect> for Action<V, R, C> {
    fn lift(_: PopEffect) -> Self {
        Action::Pop
    }
}

/// [`PushEffect`] lifts to [`Action::Push`] when `C` can be lifted from `E`.
impl<V, R, C, E> Lift<PushEffect<E>> for Action<V, R, C>
where
    C: Lift<E>,
{
    fn lift(from: PushEffect<E>) -> Self {
        Action::Push(Lift::lift(from.0))
    }
}

// ---------------------------------------------------------------------------
// Boxed — owned cursor erased to dyn Execute<I>
// ---------------------------------------------------------------------------

/// An owned type-erased cursor for [`MultiStage`]'s heterogeneous cursor stack.
///
/// `Boxed<'ir, I>` wraps `Box<dyn Execute<I> + 'ir>` but deliberately does NOT
/// implement `Execute<I>` itself. This avoids a coherence conflict: a blanket
/// `impl<T: Execute<I>> Lift<T> for Boxed<'ir, I>` would overlap with
/// `impl<T> Lift<T> for T` if a downstream crate added
/// `impl Execute<DownstreamI> for Boxed<DownstreamI>` (allowed by orphan rules
/// since `DownstreamI` is local to that crate). Instead, dialect crates provide
/// explicit `impl Lift<TheirCursor> for Boxed<'ir, MultiStage<S>>` using the
/// fact that their cursor type is local to them.
pub struct Boxed<'ir, I>(pub Box<dyn Execute<I> + 'ir>);

impl<'ir, I> Boxed<'ir, I>
where
    I: Machine + ValueStore<Error = <I as Machine>::Error> + PipelineAccess,
{
    pub fn execute(
        &mut self,
        interp: &mut I,
    ) -> Result<<I as Machine>::Effect, <I as Machine>::Error> {
        self.0.execute(interp)
    }
}

// ---------------------------------------------------------------------------
// SingleStage
// ---------------------------------------------------------------------------

/// Single-stage concrete interpreter.
///
/// Executes statements in a single dialect `L` at one compilation stage.
/// `M` is an optional inner dialect machine (default `()`) that handles
/// delegated effects. `C` is the cursor entry type (default `BlockCursor<V>`).
///
/// The interpreter maintains a global cursor stack (`Vec<C>`) instead of a
/// single cursor. The driver loop pops the top entry, calls
/// [`Execute::execute`], and dispatches the resulting [`Action`].
pub struct SingleStage<'ir, L: Dialect, V: Clone, M = (), C = BlockCursor<V, L>> {
    pipeline: &'ir Pipeline<StageInfo<L>>,
    stage_id: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<C>,
    machine: M,
    pending_yield: Option<V>,
}

// -- Machine (unit) ---------------------------------------------------------

/// `()` as a machine accepts no effects and does nothing.
impl Machine for () {
    type Effect = ();
    type Error = InterpreterError;

    fn consume_effect(&mut self, _: ()) -> Result<(), InterpreterError> {
        Ok(())
    }
}

// -- Machine ----------------------------------------------------------------

impl<'ir, L, V, M, C> Machine for SingleStage<'ir, L, V, M, C>
where
    L: Dialect,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    type Effect = Action<V, M::Effect, C>;
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Action<V, M::Effect, C>) -> Result<(), InterpreterError> {
        match effect {
            Action::Delegate(inner) => self.machine.consume_effect(inner),
            _ => Err(InterpreterError::UnhandledEffect(
                "structural effects must be handled by the driver".into(),
            )),
        }
    }
}

// -- ValueStore -------------------------------------------------------------

impl<L: Dialect, V: Clone, M, C> ValueStore for SingleStage<'_, L, V, M, C> {
    type Value = V;
    type Error = InterpreterError;

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

// -- PipelineAccess ---------------------------------------------------------

impl<'ir, L: Dialect, V: Clone, M, C> PipelineAccess for SingleStage<'ir, L, V, M, C> {
    type StageInfo = StageInfo<L>;

    fn pipeline(&self) -> &Pipeline<StageInfo<L>> {
        self.pipeline
    }

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.stage_id)
    }
}

// -- Constructor & helpers --------------------------------------------------

impl<'ir, L: Dialect, V: Clone, M, C> SingleStage<'ir, L, V, M, C> {
    /// Create a new single-stage interpreter with an inner dialect machine.
    pub fn new(pipeline: &'ir Pipeline<StageInfo<L>>, stage_id: CompileStage, machine: M) -> Self {
        Self {
            pipeline,
            stage_id,
            frames: FrameStack::new(),
            cursors: Vec::new(),
            machine,
            pending_yield: None,
        }
    }

    /// Get a reference to the inner dialect machine.
    pub fn machine(&self) -> &M {
        &self.machine
    }

    /// Get a mutable reference to the inner dialect machine.
    pub fn machine_mut(&mut self) -> &mut M {
        &mut self.machine
    }

    /// Project to a sub-machine by shared reference.
    pub fn project_machine<Sub: ?Sized>(&self) -> &Sub
    where
        M: ProjectRef<Sub>,
    {
        self.machine.project_ref()
    }

    /// Project to a sub-machine by mutable reference.
    pub fn project_machine_mut<Sub: ?Sized>(&mut self) -> &mut Sub
    where
        M: ProjectMut<Sub>,
    {
        self.machine.project_mut()
    }

    /// Get the stage info reference for the current stage.
    pub fn stage_info(&self) -> &'ir StageInfo<L> {
        self.pipeline
            .stage(self.current_stage())
            .expect("stage must exist in pipeline")
    }

    /// Take a pending yield value, if any.
    pub fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }

    /// Push a cursor entry onto the global cursor stack.
    pub fn push_cursor(&mut self, entry: C) {
        self.cursors.push(entry);
    }

    /// Bind block argument SSA values to the provided values.
    pub fn bind_block_args(&mut self, block: Block, args: &[V]) -> Result<(), InterpreterError> {
        let stage = self.stage_info();
        let block_info = block.expect_info(stage);
        let expected = block_info.arguments.len();

        if args.len() != expected {
            return Err(InterpreterError::ArityMismatch {
                expected,
                got: args.len(),
            });
        }

        // Collect SSA keys first to release the borrow on stage info.
        let ssa_keys: Vec<SSAValue> = block_info
            .arguments
            .iter()
            .map(|ba| SSAValue::from(*ba))
            .collect();

        for (ssa, value) in ssa_keys.into_iter().zip(args.iter()) {
            self.frames.write_ssa(ssa, value.clone())?;
        }

        Ok(())
    }
}

// -- enter_function ---------------------------------------------------------

impl<'ir, L, V, M, C> SingleStage<'ir, L, V, M, C>
where
    L: Dialect,
    V: Clone,
    C: Lift<BlockCursor<V, L>>,
{
    /// Enter a function: push a frame with a [`BlockCursor`] positioned at
    /// the entry block. Block arguments are carried by the cursor and bound
    /// on first [`Execute::execute`] call.
    pub fn enter_function(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError> {
        let stage = self.stage_info();
        let frame = Frame::new(callee, self.stage_id, vec![]);
        self.frames.push(frame)?;
        let cursor = BlockCursor::new(stage, entry_block, args.to_vec(), vec![]);
        self.cursors.push(Lift::lift(cursor));
        Ok(())
    }
}

// -- Driver loop (step / run) -----------------------------------------------

impl<'ir, L, V, M, C> SingleStage<'ir, L, V, M, C>
where
    L: Dialect,
    V: Clone,
    M: Machine<Error = InterpreterError>,
    C: Execute<Self> + Lift<BlockCursor<V, L>>,
{
    /// Execute one step of the driver loop.
    ///
    /// Pops the top cursor entry, calls [`Execute::execute`], and dispatches
    /// the resulting [`Action`]. Returns `true` if a step was executed,
    /// `false` if the cursor stack is empty (execution complete).
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut entry) = self.cursors.pop() else {
            return Ok(false);
        };

        let effect = entry.execute(self)?;

        match effect {
            Action::Push(new_entry) => {
                self.cursors.push(entry);
                self.cursors.push(new_entry);
            }
            Action::Yield(v) => {
                self.pending_yield = Some(v);
            }
            Action::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    // Top-level return → treat as yield.
                    self.pending_yield = Some(v);
                } else {
                    for result in &caller_results {
                        self.frames.write(*result, v.clone())?;
                    }
                }
            }
            Action::Call(callee, _callee_stage, args, results) => {
                self.cursors.push(entry);
                self.push_call_frame(callee, args, results)?;
            }
            Action::Pop => {
                // Cursor self-removes. Drop entry, no side effects.
            }
            Action::Advance => {
                self.cursors.push(entry);
            }
            Action::Jump(_, _) => {
                self.cursors.push(entry);
            }
            Action::Delegate(inner) => {
                self.machine.consume_effect(inner)?;
                self.cursors.push(entry);
            }
        }

        Ok(true)
    }

    /// Run until the cursor stack is empty or a yield is produced.
    pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
        while self.step()? {}
        Ok(self.pending_yield.take())
    }
}

// -- push_call_frame (requires C: Lift<BlockCursor<V, L>>) ---------------------

impl<'ir, L, V, M, C> SingleStage<'ir, L, V, M, C>
where
    L: Dialect,
    V: Clone,
    M: Machine<Error = InterpreterError>,
    C: Lift<BlockCursor<V, L>>,
{
    /// Push a new call frame for the given callee, entering its entry block.
    fn push_call_frame(
        &mut self,
        callee: SpecializedFunction,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<(), InterpreterError> {
        let stage = self.stage_info();
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

        let cursor = BlockCursor::new(stage, entry_block, args, vec![]);
        self.cursors.push(Lift::lift(cursor));
        Ok(())
    }
}

// ===========================================================================
// MultiStage
// ===========================================================================

/// Multi-stage concrete interpreter.
///
/// Executes programs that span multiple dialects/stages. Each `BlockCursor<V, L>`
/// is erased to [`Boxed`] so the cursor stack is heterogeneous.
/// The `push_call_frame` method uses `StageDispatch` to create the right
/// `BlockCursor<V, L>` for the callee's stage.
///
/// # Cursor stack design
///
/// [`Boxed<'ir, Self>`] (a newtype over `Box<dyn Execute<Self> + 'ir>`) is used
/// instead of a closed enum so that new cursor types (SCF cursors, user-defined
/// seeds, etc.) can be added from any crate without modifying `MultiStage`.
/// The vtable overhead and per-call heap allocation are negligible relative to
/// IR traversal work. If allocation cost ever matters, replace the inner `Box`
/// with an arena-allocated cursor pool while keeping the `dyn Execute` interface.
pub struct MultiStage<'ir, S: StageMeta, V: Clone, M = ()> {
    pipeline: &'ir Pipeline<S>,
    root_stage: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<Boxed<'ir, Self>>,
    machine: M,
    pending_yield: Option<V>,
}

// -- Machine ----------------------------------------------------------------

impl<'ir, S, V, M> Machine for MultiStage<'ir, S, V, M>
where
    S: StageMeta,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    type Effect = Action<V, M::Effect, Boxed<'ir, Self>>;
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Action<V, M::Effect, Boxed<'ir, Self>>,
    ) -> Result<(), InterpreterError> {
        match effect {
            Action::Delegate(inner) => self.machine.consume_effect(inner),
            _ => Err(InterpreterError::UnhandledEffect(
                "structural effects must be handled by the driver".into(),
            )),
        }
    }
}

// -- ValueStore -------------------------------------------------------------

impl<S: StageMeta, V: Clone, M> ValueStore for MultiStage<'_, S, V, M> {
    type Value = V;
    type Error = InterpreterError;

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

// -- PipelineAccess ---------------------------------------------------------

impl<'ir, S: StageMeta, V: Clone, M> PipelineAccess for MultiStage<'ir, S, V, M> {
    type StageInfo = S;

    fn pipeline(&self) -> &Pipeline<S> {
        self.pipeline
    }

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.root_stage)
    }
}

// -- Constructor & helpers --------------------------------------------------

impl<'ir, S: StageMeta, V: Clone, M> MultiStage<'ir, S, V, M> {
    pub fn new(pipeline: &'ir Pipeline<S>, root_stage: CompileStage, machine: M) -> Self {
        Self {
            pipeline,
            root_stage,
            frames: FrameStack::new(),
            cursors: Vec::new(),
            machine,
            pending_yield: None,
        }
    }

    pub fn machine(&self) -> &M {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut M {
        &mut self.machine
    }

    pub fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

impl<'ir, S, V, M> MultiStage<'ir, S, V, M>
where
    S: StageMeta,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    /// Enter a function at a specific dialect stage `L`.
    pub fn enter_function<L>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        L: Dialect,
        S: HasStageInfo<L>,
        BlockCursor<V, L>: Execute<Self> + 'ir,
    {
        let cursor: BlockCursor<V, L> = {
            let stage = self
                .current_stage_info::<L>()
                .ok_or(InterpreterError::MissingEntry)?;
            BlockCursor::new(stage, entry_block, args.to_vec(), vec![])
        };
        let frame = Frame::new(callee, self.root_stage, vec![]);
        self.frames.push(frame)?;
        self.cursors.push(Boxed(Box::new(cursor)));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MakeCursorAction — creates a BlockCursor<V, L> via StageDispatch
// ---------------------------------------------------------------------------

pub struct MakeCursorAction<'ir, S: StageMeta, V: Clone, M> {
    callee: SpecializedFunction,
    args: Vec<V>,
    cursor: Option<Boxed<'ir, MultiStage<'ir, S, V, M>>>,
}

impl<'ir, S, L, V, M> StageAction<S, L> for MakeCursorAction<'ir, S, V, M>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    BlockCursor<V, L>: Execute<MultiStage<'ir, S, V, M>> + 'ir,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    type Output = ();
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage_id: CompileStage,
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

        let cursor = BlockCursor::<V, L>::new(stage, entry_block, self.args.clone(), vec![]);
        self.cursor = Some(Boxed(Box::new(cursor)));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MakeBlockCursorAction — creates a BlockCursor<V, L> from a Block
// ---------------------------------------------------------------------------

/// Like [`MakeCursorAction`] but starts from a [`Block`] rather than a
/// [`SpecializedFunction`]. Used by SCF-style cursors that push inline blocks
/// (loop bodies, if-branches) onto [`MultiStage`]'s cursor stack.
pub struct MakeBlockCursorAction<'ir, S: StageMeta, V: Clone, M> {
    pub block: Block,
    pub args: Vec<V>,
    /// Populated by `run`; consumed by the caller.
    pub cursor: Option<Boxed<'ir, MultiStage<'ir, S, V, M>>>,
}

impl<'ir, S, L, V, M> StageAction<S, L> for MakeBlockCursorAction<'ir, S, V, M>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    BlockCursor<V, L>: Execute<MultiStage<'ir, S, V, M>> + 'ir,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    type Output = ();
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage_id: CompileStage,
        stage: &StageInfo<L>,
    ) -> Result<(), InterpreterError> {
        let cursor = BlockCursor::<V, L>::new(stage, self.block, self.args.clone(), vec![]);
        self.cursor = Some(Boxed(Box::new(cursor)));
        Ok(())
    }
}

// -- Driver loop (step / run) -----------------------------------------------

impl<'ir, S, V, M> MultiStage<'ir, S, V, M>
where
    S: StageMeta + SupportsStageDispatch<MakeCursorAction<'ir, S, V, M>, (), InterpreterError>,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut entry) = self.cursors.pop() else {
            return Ok(false);
        };

        let effect = entry.execute(self)?;

        match effect {
            Action::Push(new_entry) => {
                self.cursors.push(entry);
                self.cursors.push(new_entry);
            }
            Action::Yield(v) => {
                self.pending_yield = Some(v);
            }
            Action::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    self.pending_yield = Some(v);
                } else {
                    for result in &caller_results {
                        self.frames.write(*result, v.clone())?;
                    }
                }
            }
            Action::Call(callee, callee_stage, args, results) => {
                self.cursors.push(entry);
                self.push_call_frame(callee, callee_stage, args, results)?;
            }
            Action::Pop => {}
            Action::Advance => {
                self.cursors.push(entry);
            }
            Action::Jump(_, _) => {
                self.cursors.push(entry);
            }
            Action::Delegate(inner) => {
                self.machine.consume_effect(inner)?;
                self.cursors.push(entry);
            }
        }

        Ok(true)
    }

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
        let stage_container = self
            .pipeline
            .stage(callee_stage)
            .ok_or(InterpreterError::MissingEntry)?;

        let mut action = MakeCursorAction {
            callee,
            args,
            cursor: None,
        };

        S::dispatch_stage_action(stage_container, callee_stage, &mut action)?
            .ok_or(InterpreterError::MissingEntry)?;

        let cursor = action.cursor.ok_or(InterpreterError::MissingEntry)?;

        let frame = Frame::new(callee, callee_stage, results);
        self.frames.push(frame)?;
        self.cursors.push(cursor);
        Ok(())
    }
}

impl<'ir, S, V, M> MultiStage<'ir, S, V, M>
where
    S: StageMeta + SupportsStageDispatch<MakeBlockCursorAction<'ir, S, V, M>, (), InterpreterError>,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    /// Create a [`Boxed`] cursor for `block` at the given stage, using
    /// `StageDispatch` to select the right dialect `L`.
    ///
    /// Used by SCF-style cursors (e.g. `IfCursor`, `ForCursor`) when they need
    /// to push an inline block onto the cursor stack without starting a new frame.
    pub fn make_block_cursor(
        &mut self,
        block: Block,
        body_stage: CompileStage,
        args: Vec<V>,
    ) -> Result<Boxed<'ir, Self>, InterpreterError> {
        let stage_container = self
            .pipeline
            .stage(body_stage)
            .ok_or(InterpreterError::MissingEntry)?;

        let mut action = MakeBlockCursorAction {
            block,
            args,
            cursor: None,
        };

        S::dispatch_stage_action(stage_container, body_stage, &mut action)?
            .ok_or(InterpreterError::MissingEntry)?;

        action.cursor.ok_or(InterpreterError::MissingEntry)
    }
}
