use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};

use crate::control::{Control, ControlExt};
use crate::cursor::{BlockCursor, Execute};
use crate::env::ConcreteEnv;
use crate::error::InterpreterError;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::interp::Interp;
use crate::lift::Lift;
use crate::pipeline::PipelineHandle;
use crate::store::Store;

/// Single-dialect concrete (cursor-stack) interpreter.
///
/// `S` — stage container type (e.g. `StageInfo<L>` for single-dialect pipelines,
///        or a multi-stage enum).
/// `L` — the dialect (or composed language type).
/// `V` — value type.
/// `C` — cursor coproduct type (e.g. `HighLevelCursor<V>`).
pub struct ConcreteInterp<'ir, S: StageMeta, L: Dialect, V: Clone, C> {
    handle: PipelineHandle<'ir, S>,
    frames: FrameStack<V>,
    cursors: Vec<C>,
    pending_yield: Option<V>,
    _phantom: PhantomData<L>,
}

// -- Store ------------------------------------------------------------------

impl<'ir, S, L, V, C> Store for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta,
    L: Dialect,
    V: Clone,
    C: 'static,
{
    type Value = V;
    type Error = InterpreterError;

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.frames.read(ssa)
    }

    fn write_result(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.frames.write(r, v)
    }

    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.frames.write_ssa(ssa, v)
    }
}

// -- Interp -----------------------------------------------------------------

impl<'ir, S, L, V, C> Interp for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: 'static,
{
    type Dialect = L;
    /// Concrete mode: cursor push/pop events are expressed as `ControlExt<C>`.
    type Ext = ControlExt<C>;
    type StageContainer = S;

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.handle.stage_id)
    }

    fn stage_info_for<LD: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<LD>>
    where
        S: HasStageInfo<LD>,
    {
        self.handle.stage_info_for::<LD>(stage_id)
    }

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError>
    where
        S: HasStageInfo<L>,
    {
        self.handle.resolve_function_for::<L>(target, stage_id)
    }
}

// -- ConcreteEnv ------------------------------------------------------------

impl<'ir, S, L, V, C> ConcreteEnv for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: 'static,
{
    type Cursor = C;

    fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

// -- Constructor ------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone, C> ConcreteInterp<'ir, S, L, V, C> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self {
            handle: PipelineHandle::new(pipeline, stage_id),
            frames: FrameStack::new(),
            cursors: Vec::new(),
            pending_yield: None,
            _phantom: PhantomData,
        }
    }
}

impl<'ir, L: Dialect, V: Clone, C> ConcreteInterp<'ir, StageInfo<L>, L, V, C> {
    /// Convenience constructor for single-dialect pipelines.
    pub fn from_single_stage(
        pipeline: &'ir Pipeline<StageInfo<L>>,
        stage_id: CompileStage,
    ) -> Self {
        Self::new(pipeline, stage_id)
    }
}

// -- enter_function ---------------------------------------------------------

impl<'ir, S, L, V, C> ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: Execute<Self> + 'static,
{
    /// Push a call frame and a `BlockCursor<V, LD>` for the entry block.
    ///
    /// `LD` is the dialect of the callee (may differ from `L` for cross-stage calls).
    pub fn enter_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        S: HasStageInfo<LD>,
        BlockCursor<V, LD>: Execute<Self> + 'static,
        C: Lift<BlockCursor<V, LD>>,
    {
        let stage_id = self.handle.stage_id;
        let cursor = BlockCursor::<V, LD>::new(entry_block, stage_id, args.to_vec());
        let frame = Frame::new(callee, stage_id, vec![]);
        self.frames.push(frame)?;
        self.cursors.push(C::lift(cursor));
        Ok(())
    }
}

// -- Driver loop ------------------------------------------------------------

impl<'ir, S, L, V, C> ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: Execute<Self> + Lift<BlockCursor<V, L>> + 'static,
{
    /// Execute one step. Returns `true` if work remains, `false` when done.
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut cursor) = self.cursors.pop() else {
            return Ok(false);
        };

        let effect: Control<V, ControlExt<C>> = cursor.execute(self)?;

        match effect {
            Control::Advance => {
                self.cursors.push(cursor);
            }
            Control::Jump(..) => {
                // BlockCursor handles Jump internally and loops back — it only
                // returns Jump if the block ends without a terminator, which
                // indicates a malformed program.
                self.cursors.push(cursor);
            }
            Control::Ext(ControlExt::Push(new_cursor)) => {
                // Push the current cursor back, then push the new one on top so
                // it executes first.
                self.cursors.push(cursor);
                self.cursors.push(new_cursor);
            }
            Control::Ext(ControlExt::Pop) => {
                // Current cursor is done; discard it (already popped).
                // The previous cursor (if any) resumes on the next step.
            }
            Control::Yield(v) => {
                self.pending_yield = Some(v);
                // Don't push cursor back — it finished its body.
            }
            Control::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    // Top-level return — deliver to the caller of `run`.
                    self.pending_yield = Some(v);
                } else {
                    // Write return value into the caller frame.
                    for result in &caller_results {
                        self.frames.write(*result, v.clone())?;
                    }
                }
            }
            Control::Call {
                callee,
                stage,
                args,
                results,
            } => {
                self.cursors.push(cursor);
                self.push_call_frame(callee, stage, args, results)?;
            }
            Control::Fork(..) => {
                return Err(InterpreterError::UnhandledEffect(
                    "Control::Fork reached concrete driver; \
                     use AbstractInterp for nondeterministic branches"
                        .into(),
                ));
            }
        }

        Ok(true)
    }

    /// Run until the cursor stack is empty or a top-level return is produced.
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
        let stage_id = self.handle.stage_id;
        let stage: &StageInfo<L> = self
            .handle
            .pipeline
            .stage(stage_id)
            .and_then(|s| s.try_stage_info())
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

        let cursor = BlockCursor::<V, L>::new(entry_block, stage_id, args);
        let frame = Frame::new(callee, stage_id, results);
        self.frames.push(frame)?;
        self.cursors.push(C::lift(cursor));
        Ok(())
    }
}
