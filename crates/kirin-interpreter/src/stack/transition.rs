use kirin_ir::{
    CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo, StageMeta, SupportsStageDispatch,
};

use super::{DynFrameDispatch, FrameDispatchAction, StackInterpreter};
use crate::{BlockEvaluator, ConcreteExt, Continuation, Interpretable, InterpreterError};

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + crate::ProductValue + 'ir,
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

    /// Look up the cached dispatch entry for `stage_id`.
    pub(super) fn lookup_dispatch(
        &self,
        stage_id: CompileStage,
    ) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E> {
        match self.dispatch_table.get(stage_id).copied() {
            Some(dispatch) => Ok(dispatch),
            None => {
                if self.pipeline.stage(stage_id).is_none() {
                    Err(InterpreterError::StageResolution {
                        stage: stage_id,
                        kind: crate::StageResolutionError::MissingStage,
                    }
                    .into())
                } else {
                    Err(InterpreterError::StageResolution {
                        stage: stage_id,
                        kind: crate::StageResolutionError::MissingDialect,
                    }
                    .into())
                }
            }
        }
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
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self> + 'ir,
    {
        self.spend_fuel()?;
        let cursor = self
            .current_cursor()?
            .ok_or_else(|| InterpreterError::NoFrame)?;
        let def: &L = cursor.definition(stage);
        def.interpret::<L>(self)
    }

    pub(super) fn advance_frame_with_stage<L>(
        &mut self,
        stage: &'ir StageInfo<L>,
        control: &Continuation<V, ConcreteExt>,
    ) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self> + 'ir,
    {
        match control {
            Continuation::Continue => {
                self.advance_cursor_in_stage::<L>(stage)?;
            }
            Continuation::Jump(block, args) => {
                self.bind_block_args(stage, *block, args)?;
                let first = block.first_statement(stage);
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
            .ok_or_else(|| InterpreterError::NoFrame)?;
        let next = *cursor.next::<L>(stage);
        if let Some(next_stmt) = next {
            self.set_current_cursor(Some(next_stmt))?;
        } else {
            let parent = *cursor.parent::<L>(stage);
            if let Some(kirin_ir::StatementParent::Block(block)) = parent {
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
