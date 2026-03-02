use kirin_ir::{
    CompileStage, Dialect, GetInfo, SpecializedFunction, StageInfo, StageMeta,
    SupportsStageDispatch,
};

use super::StackInterpreter;
use super::dispatch::CallDynAction;
use crate::stage::expect_stage_id;
use crate::{BlockEvaluator, CallSemantics, Continuation, Frame, Interpretable, InterpreterError};

// -- Call (inherent, not on the trait) --------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
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
        crate::dispatch::dispatch_in_pipeline(pipeline, stage, &mut action)
    }

    pub(super) fn call_with_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<V, E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + CallSemantics<'ir, Self, L, Result = V> + 'ir,
    {
        let stage_id = expect_stage_id(stage);
        let spec = callee
            .get_info(stage)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: crate::StageResolutionError::MissingCallee { callee },
            })?;
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);
        def.eval_call(self, stage, callee, args)
    }

    pub(super) fn push_call_frame_with_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<(), E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage_id = expect_stage_id(stage);
        let spec = callee
            .get_info(stage)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: crate::StageResolutionError::MissingCallee { callee },
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
        if let Err(err) = self.bind_block_args(stage, entry_block, args) {
            let _ = self.pop_frame();
            return Err(err);
        }
        Ok(())
    }

    /// Push a call frame dynamically using the pre-built dispatch table.
    pub(super) fn push_call_frame_with_args(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: &[V],
    ) -> Result<(), E> {
        let dispatch = self.lookup_dispatch(stage_id)?;
        (dispatch.push_call_frame)(self, stage_id, callee, args)
    }
}
