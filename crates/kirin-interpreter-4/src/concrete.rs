use crate::cursor::BlockCursor;
use crate::effect::CursorEffect;
use crate::error::InterpreterError;
use crate::execute::Execute;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::lift::{Lift, ProjectMut, ProjectRef};
use crate::traits::{Machine, PipelineAccess, ValueStore};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, Pipeline, ResultValue, SSAValue, SpecializedFunction,
    StageInfo,
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
    /// Call a specialized function with arguments, writing results to the given slots.
    Call(SpecializedFunction, Vec<V>, Vec<ResultValue>),
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
pub struct SingleStage<'ir, L: Dialect, V: Clone, M = (), C = BlockCursor<V>> {
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
    C: Lift<BlockCursor<V>>,
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
    C: Execute<Self> + Lift<BlockCursor<V>>,
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
            Action::Call(callee, args, results) => {
                self.cursors.push(entry);
                self.push_call_frame(callee, args, results)?;
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

// -- push_call_frame (requires C: Lift<BlockCursor<V>>) ---------------------

impl<'ir, L, V, M, C> SingleStage<'ir, L, V, M, C>
where
    L: Dialect,
    V: Clone,
    M: Machine<Error = InterpreterError>,
    C: Lift<BlockCursor<V>>,
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
