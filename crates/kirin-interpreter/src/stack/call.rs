use kirin_ir::{
    CompileStage, Dialect, GetInfo, HasStageInfo, SpecializedFunction, StageInfo, StageMeta,
    SupportsStageDispatch,
};

use super::dispatch::{CallDynAction, PushCallFrameDynAction};
use super::{DynFrameDispatch, FrameDispatchAction, StackInterpreter};
use crate::{Continuation, EvalCall, Frame, Interpretable, Interpreter, InterpreterError};

// -- Call (inherent, not on the trait) --------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Call a specialized function and return its result value using strict
    /// typed-stage checking.
    pub fn call_in_stage<L>(&mut self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V> + 'ir,
    {
        let stage_id = self.active_stage();
        self.call_with_stage_id::<L>(callee, stage_id, args)
    }

    /// Stage-dynamic call entrypoint. The target dialect is resolved at
    /// runtime from stage metadata.
    pub fn call(
        &mut self,
        callee: SpecializedFunction,
        stage: CompileStage,
        args: &[V],
    ) -> Result<V, E>
    where
        for<'a> S: SupportsStageDispatch<CallDynAction<'a, 'ir, V, S, E, G>, V, E>,
    {
        let pipeline = self.pipeline;
        let mut action = CallDynAction {
            interp: self,
            callee,
            args,
        };
        Self::dispatch_in_pipeline(pipeline, stage, &mut action)
    }

    pub(super) fn call_with_stage_id<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: &[V],
    ) -> Result<V, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V> + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;
        self.call_in_resolved_stage::<L>(callee, stage_id, stage, args)
    }

    fn call_in_resolved_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<V, E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V> + 'ir,
    {
        let spec =
            callee
                .get_info(stage)
                .ok_or_else(|| InterpreterError::MissingCalleeAtStage {
                    callee,
                    stage: stage_id,
                })?;
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);
        def.eval_call(self, stage, callee, args)
    }

    pub(super) fn push_call_frame_with_args(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: &[V],
    ) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        let pipeline = self.pipeline;
        let mut action = PushCallFrameDynAction::new(self, callee, args);
        Self::dispatch_in_pipeline(pipeline, stage_id, &mut action)
    }

    pub(super) fn push_call_frame_in_resolved_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let spec =
            callee
                .get_info(stage)
                .ok_or_else(|| InterpreterError::MissingCalleeAtStage {
                    callee,
                    stage: stage_id,
                })?;
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);

        // Push callee frame first so `active_stage()` during body interpretation
        // resolves to the callee stage without mutable stage tracking on the
        // interpreter state.
        self.push_frame(Frame::new(callee, stage_id, None))?;
        let entry_block = match def.interpret(self) {
            Ok(Continuation::Jump(succ, _)) => succ.target(),
            Ok(_) => {
                let _ = self.pop_frame();
                return Err(InterpreterError::MissingEntry.into());
            }
            Err(err) => {
                let _ = self.pop_frame();
                return Err(err);
            }
        };

        let first = entry_block.first_statement(stage);
        self.set_current_cursor(first)?;
        if let Err(err) = self.bind_block_args_in_stage::<L>(stage, entry_block, args) {
            let _ = self.pop_frame();
            return Err(err);
        }
        Ok(())
    }
}
