use crate::cursor::BlockCursor;
use crate::effect::CursorEffect;
use crate::error::InterpreterError;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::lift::{Lift, LiftInto};
use crate::traits::{Interpretable, Machine, PipelineAccess, ValueStore};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, Pipeline, ResultValue, SSAValue, SpecializedFunction,
    StageInfo, Statement,
};

// ---------------------------------------------------------------------------
// Action — the interpreter's effect algebra
// ---------------------------------------------------------------------------

/// The interpreter's own effect type.
///
/// Dialect effects are lifted into `Action` via [`Lift`]. The interpreter's
/// [`Machine::consume_effect`] dispatches on these variants directly.
pub enum Action<V, R = ()> {
    /// Advance to the next statement in the current block.
    Advance,
    /// Jump the cursor to a different block with the given arguments.
    Jump(Block, Vec<V>),
    /// Delegate to the inner dialect machine.
    Delegate(R),
}

// -- Lift impls: dialect effects → Action -----------------------------------

/// `()` (no effect) lifts to `Advance`.
impl<V, R> Lift<()> for Action<V, R> {
    fn lift(_: ()) -> Self {
        Action::Advance
    }
}

/// [`CursorEffect`] lifts directly into the corresponding [`Action`] variant.
impl<V, R> Lift<CursorEffect<V>> for Action<V, R> {
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
/// delegated effects.
///
/// The interpreter's [`Machine::Effect`] is `Action<V, M::Effect>`. Dialect
/// effects are lifted into `Action` via [`Lift`], then consumed by the
/// interpreter: cursor effects are handled directly, and the remainder is
/// delegated to the inner machine.
pub struct SingleStage<'ir, L: Dialect, V: Clone, M = ()> {
    pipeline: &'ir Pipeline<StageInfo<L>>,
    stage_id: CompileStage,
    frames: FrameStack<V, BlockCursor>,
    machine: M,
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

impl<'ir, L, V, M> Machine for SingleStage<'ir, L, V, M>
where
    L: Dialect,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    type Effect = Action<V, M::Effect>;
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Action<V, M::Effect>) -> Result<(), InterpreterError> {
        match effect {
            Action::Advance => self.advance_cursor(),
            Action::Jump(block, args) => self.jump_to_block(block, &args),
            Action::Delegate(inner) => self.machine.consume_effect(inner),
        }
    }
}

// -- ValueStore -------------------------------------------------------------

impl<L: Dialect, V: Clone, M> ValueStore for SingleStage<'_, L, V, M> {
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

impl<'ir, L: Dialect, V: Clone, M> PipelineAccess for SingleStage<'ir, L, V, M> {
    type StageInfo = StageInfo<L>;

    fn pipeline(&self) -> &Pipeline<StageInfo<L>> {
        self.pipeline
    }

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.stage_id)
    }
}

// With Machine + ValueStore + PipelineAccess, the blanket impl gives us Interpreter.

// -- Constructor & helpers --------------------------------------------------

impl<'ir, L: Dialect, V: Clone, M> SingleStage<'ir, L, V, M> {
    /// Create a new single-stage interpreter with an inner dialect machine.
    pub fn new(pipeline: &'ir Pipeline<StageInfo<L>>, stage_id: CompileStage, machine: M) -> Self {
        Self {
            pipeline,
            stage_id,
            frames: FrameStack::new(),
            machine,
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

    /// Get the stage info reference for the current stage.
    fn stage_info(&self) -> &'ir StageInfo<L> {
        self.pipeline
            .stage(self.current_stage())
            .expect("stage must exist in pipeline")
    }

    /// Get the current statement from the top frame's cursor.
    pub fn current_statement(&self) -> Option<Statement> {
        self.frames.current()?.extra().current()
    }

    /// Enter a function: push a frame with a [`BlockCursor`] positioned at
    /// the entry block, then bind the block arguments.
    pub fn enter_function(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError> {
        let stage = self.stage_info();
        let cursor = BlockCursor::new(stage, entry_block);
        let frame = Frame::new(callee, self.stage_id, cursor);
        self.frames.push(frame)?;
        self.bind_block_args(entry_block, args)?;
        Ok(())
    }

    /// Bind block argument SSA values to the provided values.
    fn bind_block_args(&mut self, block: Block, args: &[V]) -> Result<(), InterpreterError> {
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

    /// Advance the cursor to the next statement in the current block.
    fn advance_cursor(&mut self) -> Result<(), InterpreterError> {
        let stage = self.stage_info();
        let frame = self.frames.current_mut().ok_or(InterpreterError::NoFrame)?;
        frame.extra_mut().advance(stage);
        Ok(())
    }

    /// Jump the cursor to a new block with arguments.
    fn jump_to_block(&mut self, block: Block, args: &[V]) -> Result<(), InterpreterError> {
        let stage = self.stage_info();
        let cursor = BlockCursor::new(stage, block);
        let frame = self.frames.current_mut().ok_or(InterpreterError::NoFrame)?;
        *frame.extra_mut() = cursor;
        self.bind_block_args(block, args)?;
        Ok(())
    }
}

// -- Step / Run -------------------------------------------------------------

impl<'ir, L, V, M> SingleStage<'ir, L, V, M>
where
    L: Dialect,
    V: Clone,
    M: Machine<Error = InterpreterError>,
{
    /// Execute one statement and handle its effect.
    ///
    /// Returns `true` if a statement was executed, `false` if no current
    /// statement (block exhausted).
    pub fn step(&mut self) -> Result<bool, InterpreterError>
    where
        L: Interpretable<Self>,
        <L as Interpretable<Self>>::Effect: LiftInto<Action<V, M::Effect>>,
        <L as Interpretable<Self>>::Error: Into<InterpreterError>,
    {
        let Some(stmt) = self.current_statement() else {
            return Ok(false);
        };

        let stage = self.stage_info();
        let definition = stmt.definition(stage);
        let effect = definition.interpret(self).map_err(Into::into)?;

        self.consume_effect(effect.lift_into())?;

        Ok(true)
    }

    /// Run until no more statements in the current frame.
    pub fn run(&mut self) -> Result<(), InterpreterError>
    where
        L: Interpretable<Self>,
        <L as Interpretable<Self>>::Effect: LiftInto<Action<V, M::Effect>>,
        <L as Interpretable<Self>>::Error: Into<InterpreterError>,
    {
        while self.step()? {}
        Ok(())
    }

    /// Execute a block inline: push a frame, run to completion, pop the frame.
    ///
    /// This is an execution seed — dialect authors call this during their
    /// [`Interpretable::interpret`] to execute a nested body synchronously.
    pub fn exec_block(
        &mut self,
        callee: SpecializedFunction,
        block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        L: Interpretable<Self>,
        <L as Interpretable<Self>>::Effect: LiftInto<Action<V, M::Effect>>,
        <L as Interpretable<Self>>::Error: Into<InterpreterError>,
    {
        self.enter_function(callee, block, args)?;
        self.run()?;
        self.frames.pop();
        Ok(())
    }
}
