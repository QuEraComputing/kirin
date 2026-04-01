use crate::cursor::BlockCursor;
use crate::effect::CursorEffect;
use crate::error::InterpreterError;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::traits::{Interpretable, Machine, PipelineAccess, ValueStore};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, Pipeline, ResultValue, SSAValue, SpecializedFunction,
    StageInfo, Statement,
};

// ---------------------------------------------------------------------------
// Action + IntoAction
// ---------------------------------------------------------------------------

/// What the interpreter should do after a statement executes.
pub enum Action<V> {
    /// Advance to the next statement in the current block.
    Advance,
    /// Jump the cursor to a different block with the given arguments.
    Jump(Block, Vec<V>),
}

/// Convert an effect into an interpreter [`Action`].
pub trait IntoAction<V> {
    fn into_action(self) -> Action<V>;
}

/// `()` means "no effect" — default to advancing the cursor.
impl<V> IntoAction<V> for () {
    fn into_action(self) -> Action<V> {
        Action::Advance
    }
}

/// [`CursorEffect`] maps directly to [`Action`].
impl<V> IntoAction<V> for CursorEffect<V> {
    fn into_action(self) -> Action<V> {
        match self {
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
/// Uses [`FrameStack<V, BlockCursor>`] where each frame holds SSA values
/// and a block cursor tracking execution position.
pub struct SingleStage<'ir, L: Dialect, V: Clone> {
    pipeline: &'ir Pipeline<StageInfo<L>>,
    stage_id: CompileStage,
    frames: FrameStack<V, BlockCursor>,
}

// -- Machine ----------------------------------------------------------------

impl<L: Dialect, V: Clone> Machine for SingleStage<'_, L, V> {
    type Effect = ();
    type Error = InterpreterError;

    fn consume_effect(&mut self, _: ()) -> Result<(), InterpreterError> {
        Ok(())
    }
}

// -- ValueStore -------------------------------------------------------------

impl<L: Dialect, V: Clone> ValueStore for SingleStage<'_, L, V> {
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

impl<'ir, L: Dialect, V: Clone> PipelineAccess for SingleStage<'ir, L, V> {
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

impl<'ir, L: Dialect, V: Clone> SingleStage<'ir, L, V> {
    /// Create a new single-stage interpreter.
    pub fn new(pipeline: &'ir Pipeline<StageInfo<L>>, stage_id: CompileStage) -> Self {
        Self {
            pipeline,
            stage_id,
            frames: FrameStack::new(),
        }
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
}

// -- Step / Run -------------------------------------------------------------

impl<'ir, L: Dialect, V: Clone> SingleStage<'ir, L, V> {
    /// Execute one statement and handle its effect.
    ///
    /// Returns `true` if a statement was executed, `false` if no current
    /// statement (block exhausted).
    pub fn step(&mut self) -> Result<bool, InterpreterError>
    where
        L: Interpretable<Self>,
        <L as Interpretable<Self>>::Effect: IntoAction<V>,
        <L as Interpretable<Self>>::Error: Into<InterpreterError>,
    {
        let Some(stmt) = self.current_statement() else {
            return Ok(false);
        };

        let stage = self.stage_info();
        let definition = stmt.definition(stage);
        let effect = definition.interpret(self).map_err(Into::into)?;

        match effect.into_action() {
            Action::Advance => self.advance_cursor()?,
            Action::Jump(block, args) => self.jump_to_block(block, &args)?,
        }

        Ok(true)
    }

    /// Run until no more statements in the current frame.
    pub fn run(&mut self) -> Result<(), InterpreterError>
    where
        L: Interpretable<Self>,
        <L as Interpretable<Self>>::Effect: IntoAction<V>,
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
        <L as Interpretable<Self>>::Effect: IntoAction<V>,
        <L as Interpretable<Self>>::Error: Into<InterpreterError>,
    {
        self.enter_function(callee, block, args)?;
        self.run()?;
        self.frames.pop();
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
