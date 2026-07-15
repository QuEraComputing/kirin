use kirin_ir::{Block, CompileStage, Product, SSAValue, Statement};

use crate::{EnvIndex, FrameDriver, InterpreterError};

/// Block-cursor mechanics shared by the block-shaped walkers
/// ([`BlockFrame`](super::BlockFrame) and [`CfgFrame`](super::CfgFrame)):
/// the current block, the statement cursor, lazily bound entry arguments,
/// and the result slots awaiting a pushed child's completion values.
///
/// Traversal state only — no environment ownership, no invocation role.
pub(super) struct BlockCursor<V> {
    pub(super) stage: CompileStage,
    pub(super) index: EnvIndex,
    pub(super) block: Block,
    cursor: Option<Statement>,
    /// Entry arguments not yet bound. A frame built by a dialect frame is
    /// constructed without engine access — it binds on its first `step`, so
    /// construction needs no [`FrameDriver`].
    pending: Option<Product<V>>,
    /// Result slots awaiting a pushed child frame's completion values.
    resume_slots: Option<Product<SSAValue>>,
}

impl<V: Clone> BlockCursor<V> {
    pub(super) fn new(
        stage: CompileStage,
        index: EnvIndex,
        block: Block,
        args: Product<V>,
    ) -> Self {
        Self {
            stage,
            index,
            block,
            cursor: None,
            pending: Some(args),
            resume_slots: None,
        }
    }

    /// Bind pending entry arguments to the block's parameters and position
    /// the cursor at its first statement. Returns `true` if binding happened
    /// on this call (the frame should `Continue` and step again).
    pub(super) fn bind_entry<I>(&mut self, interp: &mut I) -> Result<bool, I::Error>
    where
        I: FrameDriver<Value = V>,
    {
        match self.pending.take() {
            Some(args) => {
                interp.bind_block_args(self.stage, self.index, self.block, &args)?;
                self.cursor = interp.first_statement(self.stage, self.block)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Take the current statement, advancing the cursor past it.
    pub(super) fn advance<I>(&mut self, interp: &I) -> Result<Option<Statement>, I::Error>
    where
        I: FrameDriver<Value = V>,
    {
        let Some(statement) = self.cursor else {
            return Ok(None);
        };
        self.cursor = interp.next_statement(self.stage, self.block, statement)?;
        Ok(Some(statement))
    }

    /// Move to `target` (a CFG jump): bind its parameters and reset the
    /// cursor to its first statement.
    pub(super) fn enter_block<I>(
        &mut self,
        interp: &mut I,
        target: Block,
        args: &Product<V>,
    ) -> Result<(), I::Error>
    where
        I: FrameDriver<Value = V>,
    {
        interp.bind_block_args(self.stage, self.index, target, args)?;
        self.cursor = interp.first_statement(self.stage, target)?;
        self.block = target;
        Ok(())
    }

    /// Stash the result slots of a `Push` until the child completes.
    pub(super) fn expect_results(&mut self, results: Product<SSAValue>) {
        self.resume_slots = Some(results);
    }

    /// Write a completed child's values into the stashed result slots.
    pub(super) fn write_child_results<I>(
        &mut self,
        interp: &mut I,
        values: Product<V>,
    ) -> Result<(), I::Error>
    where
        I: FrameDriver<Value = V>,
        I::Error: From<InterpreterError>,
    {
        let slots = self.resume_slots.take().ok_or_else(|| {
            I::Error::from(InterpreterError::Custom("body resume without result slots"))
        })?;
        interp.write_results(self.index, &slots, values)
    }
}
