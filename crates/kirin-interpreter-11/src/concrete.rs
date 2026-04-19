use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta,
};

use crate::algebra::Lift;
use crate::call_dispatch::CallDispatch;
use crate::control::{Control, CursorExt};
use crate::cursor::BlockCursor;
use crate::env::{ConcreteMode, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::pipeline::PipelineHandle;

/// Single-dialect concrete (cursor-stack) interpreter.
///
/// `S` — stage container type (e.g. `Stage` from toy-lang or `StageInfo<L>`).
/// `L` — the dialect (or composed language type).
/// `V` — value type.
/// `C` — cursor coproduct type. No `'static` bound required — cursors only store
///        IR handles (Copy types) so they are naturally bounded by the IR lifetime.
pub struct ConcreteInterp<'ir, S: StageMeta, L: Dialect, V: Clone, C> {
    handle: PipelineHandle<'ir, S>,
    frames: FrameStack<V>,
    cursors: Vec<StackEntry<C, V>>,
    result: Option<V>,
    _phantom: PhantomData<L>,
}

// -- Env -----------------------------------------------------------------

impl<'ir, S, L, V, C> Env for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
{
    type Mode = ConcreteMode<C>;
    type Value = V;
    type Ext = CursorExt<C>;
    type Error = InterpreterError;
    type Stages = S;

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.handle.stage_id)
    }

    fn pipeline(&self) -> &Pipeline<S> {
        self.handle.pipeline
    }

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

// -- Constructor ------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone, C> ConcreteInterp<'ir, S, L, V, C> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self {
            handle: PipelineHandle::new(pipeline, stage_id),
            frames: FrameStack::new(),
            cursors: Vec::new(),
            result: None,
            _phantom: PhantomData,
        }
    }
}

impl<'ir, L: Dialect, V: Clone, C> ConcreteInterp<'ir, StageInfo<L>, L, V, C> {
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
    C: Execute<Self>,
{
    pub fn enter_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        S: HasStageInfo<LD>,
        BlockCursor<V, LD>: Execute<Self> + Lift<C>,
    {
        let stage_id = self.handle.stage_id;
        let cursor = BlockCursor::<V, LD>::new(entry_block, stage_id, args.to_vec());
        let frame = Frame::new(callee, stage_id, vec![]);
        self.frames.push(frame)?;
        self.cursors.push(StackEntry::new(cursor.lift()));
        Ok(())
    }
}

// -- Driver loop ------------------------------------------------------------

impl<'ir, S, L, V, C> ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + CallDispatch<V, C>,
    L: Dialect,
    V: Clone,
    C: Execute<Self>,
{
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut entry) = self.cursors.pop() else {
            return Ok(false);
        };

        let inbox = entry.inbox.take();
        let effect: Control<V, CursorExt<C>> = entry.cursor.execute(self, inbox)?;

        match effect {
            Control::Advance => {
                self.cursors.push(entry);
            }
            Control::Jump(..) => {
                // BlockCursor handles Jump internally; if it bubbles up, treat as Advance.
                self.cursors.push(entry);
            }
            Control::Ext(CursorExt::Push(new_cursor)) => {
                self.cursors.push(entry);
                self.cursors.push(StackEntry::new(new_cursor));
            }
            Control::Ext(CursorExt::Pop) => {
                // Current cursor done; discard it.
            }
            Control::Yield(v) => {
                if let Some(parent) = self.cursors.last_mut() {
                    parent.inbox = Some(v);
                } else {
                    self.result = Some(v);
                }
            }
            Control::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    self.result = Some(v);
                } else {
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
                self.cursors.push(entry);
                self.push_call_frame(callee, stage, args, results)?;
            }
            Control::Fork(..) => {
                return Err(InterpreterError::UnhandledEffect(
                    "Control::Fork in concrete interpreter; use AbstractInterp for nondeterminism"
                        .into(),
                ));
            }
        }

        Ok(true)
    }

    pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
        while self.step()? {}
        Ok(self.result.take())
    }

    /// Push a new call frame, dispatching cursor creation via `S::make_call_cursor`.
    ///
    /// `callee_stage` is where the callee lives — may differ from the interpreter's
    /// primary stage in multi-stage pipelines. `CallDispatch` selects the correct
    /// cursor variant so the callee's ops run under the right `Interpretable` impl.
    fn push_call_frame(
        &mut self,
        callee: SpecializedFunction,
        callee_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<(), InterpreterError> {
        let cursor = S::make_call_cursor(self.handle.pipeline, callee, callee_stage, args)?;
        let frame = Frame::new(callee, callee_stage, results);
        self.frames.push(frame)?;
        self.cursors.push(StackEntry::new(cursor));
        Ok(())
    }
}
