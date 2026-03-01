use kirin_ir::{CompileStage, Dialect, Pipeline, StageInfo, StageMeta, SupportsStageDispatch};

use super::{DynFrameDispatch, FrameDispatchAction, StackInterpreter};
use crate::{ConcreteExt, Continuation, Interpretable, Interpreter, InterpreterError};

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    pub(super) fn resolve_dispatch_for_stage_in_pipeline(
        pipeline: &'ir Pipeline<S>,
        stage_id: CompileStage,
    ) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
    {
        let mut action = FrameDispatchAction::new();
        crate::dispatch::dispatch_in_pipeline(pipeline, stage_id, &mut action)
    }

    pub(super) fn resolve_dispatch_for_stage(
        &self,
        stage_id: CompileStage,
    ) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
    {
        let idx = kirin_ir::Id::from(stage_id).raw();
        match self.dispatch_table.by_stage.get(idx).copied().flatten() {
            Some(dispatch) => Ok(dispatch),
            None => {
                if self.pipeline.stage(stage_id).is_none() {
                    Err(InterpreterError::MissingStage { stage: stage_id }.into())
                } else {
                    Err(InterpreterError::MissingStageDialect { stage: stage_id }.into())
                }
            }
        }
    }

    /// Look up the cached dispatch entry for `stage_id` without requiring
    /// `SupportsStageDispatch` bounds.
    pub(super) fn lookup_dispatch_cached(
        &self,
        stage_id: CompileStage,
    ) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E> {
        let idx = kirin_ir::Id::from(stage_id).raw();
        match self.dispatch_table.by_stage.get(idx).copied().flatten() {
            Some(dispatch) => Ok(dispatch),
            None => {
                if self.pipeline.stage(stage_id).is_none() {
                    Err(InterpreterError::MissingStage { stage: stage_id }.into())
                } else {
                    Err(InterpreterError::MissingStageDialect { stage: stage_id }.into())
                }
            }
        }
    }

    /// Push a call frame using only the pre-built dispatch table (no
    /// `SupportsStageDispatch` bounds needed).
    pub(super) fn push_frame_cached(
        &mut self,
        frame: crate::Frame<V, Option<kirin_ir::Statement>>,
    ) -> Result<(), E> {
        let dispatch = self.lookup_dispatch_cached(frame.stage())?;
        let internal = Self::public_frame_to_internal(frame, dispatch);
        self.frames.push(internal)?;
        Ok(())
    }

    fn spend_fuel(&mut self) -> Result<(), E> {
        if let Some(ref mut fuel) = self.fuel {
            if *fuel == 0 {
                return Err(InterpreterError::FuelExhausted.into());
            }
            *fuel -= 1;
        }
        Ok(())
    }

    pub(super) fn step_with_stage<L>(
        &mut self,
        stage: &'ir StageInfo<L>,
    ) -> Result<Continuation<V, ConcreteExt>, E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        self.spend_fuel()?;
        let cursor = self
            .current_cursor()?
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let def: &L = cursor.definition(stage);
        def.interpret(self)
    }

    pub(super) fn advance_frame_with_stage<L>(
        &mut self,
        stage: &'ir StageInfo<L>,
        control: &Continuation<V, ConcreteExt>,
    ) -> Result<(), E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        match control {
            Continuation::Continue => {
                self.advance_cursor_in_stage::<L>(stage)?;
            }
            Continuation::Jump(succ, args) => {
                self.bind_block_args(stage, succ.target(), args)?;
                let first = succ.target().first_statement(stage);
                self.set_current_cursor(first)?;
            }
            Continuation::Fork(_) => {
                return Err(InterpreterError::UnexpectedControl(
                    "Fork is not supported by concrete interpreters".to_owned(),
                )
                .into());
            }
            Continuation::Call { .. } => {
                self.advance_cursor_in_stage::<L>(stage)?;
            }
            Continuation::Return(_) => {
                self.pop_frame()?;
            }
            Continuation::Yield(_) => {}
            Continuation::Ext(ConcreteExt::Break | ConcreteExt::Halt) => {}
        }
        Ok(())
    }

    fn advance_cursor_in_stage<L>(&mut self, stage: &StageInfo<L>) -> Result<(), E>
    where
        L: Dialect,
    {
        let cursor = self
            .current_cursor()?
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let next = *cursor.next::<L>(stage);
        if let Some(next_stmt) = next {
            self.set_current_cursor(Some(next_stmt))?;
        } else {
            let parent_block = *cursor.parent::<L>(stage);
            if let Some(block) = parent_block {
                let term = block.terminator::<L>(stage);
                if term == Some(cursor) {
                    self.set_current_cursor(None)?;
                } else if let Some(t) = term {
                    self.set_current_cursor(Some(t))?;
                } else {
                    self.set_current_cursor(None)?;
                }
            } else {
                self.set_current_cursor(None)?;
            }
        }
        Ok(())
    }
}
