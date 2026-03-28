use kirin_ir::{
    Block, CompileStage, Function, GetInfo, ResultValue, SSAValue, SpecializedFunction,
    StagedFunction, Statement, Symbol,
};

use super::{
    SingleStage,
    activation::{Activation, Continuation},
};
use crate::{
    BlockSeed, Frame, FrameStack, InterpreterError, ProductValue, StageAccess,
    StageResolutionError, ValueStore,
    control::Directive,
    cursor::{ExecutionCursor, InternalBlockSeed, InternalSeed},
    interpreter::Position,
};

impl<'ir, L, V, M, E> SingleStage<'ir, L, V, M, E>
where
    L: kirin_ir::Dialect + 'ir,
    V: 'ir,
    M: crate::Machine<'ir> + 'ir,
    E: 'ir,
{
    pub(crate) fn clear_frames(&mut self) {
        self.frames = FrameStack::new();
    }

    pub(crate) fn stage_info_for(&self, stage: CompileStage) -> &'ir kirin_ir::StageInfo<L> {
        self.pipeline
            .stage(stage)
            .expect("single-stage interpreter points at a missing stage")
    }

    pub(crate) fn current_activation_mut(&mut self) -> Result<&mut Activation, InterpreterError> {
        Ok(self.frames.current_mut::<InterpreterError>()?.extra_mut())
    }

    fn activation_for(
        &self,
        stage: CompileStage,
        seed: InternalSeed,
        continuation: Option<Continuation>,
    ) -> Activation {
        let cursor = ExecutionCursor::from_seed(self.stage_info_for(stage), seed);
        Activation::new(cursor, continuation)
    }

    pub(super) fn push_frame(
        &mut self,
        callee: SpecializedFunction,
        stage: CompileStage,
        seed: InternalSeed,
        continuation: Option<Continuation>,
    ) -> Result<(), InterpreterError> {
        let activation = self.activation_for(stage, seed, continuation);
        self.frames.push(Frame::new(callee, stage, activation))
    }

    fn replace_current_seed(&mut self, seed: InternalSeed) -> Result<(), InterpreterError> {
        let stage = self.active_stage();
        let next_cursor = ExecutionCursor::from_seed(self.stage_info_for(stage), seed);
        let activation = self.current_activation_mut()?;
        activation.after_statement = None;
        let cursor = activation
            .cursor_stack
            .last_mut()
            .ok_or(InterpreterError::InvalidControl(
                "replace requires an active cursor",
            ))?;
        *cursor = next_cursor;
        Ok(())
    }

    pub(crate) fn clear_after_statement(&mut self) {
        if let Ok(activation) = self.current_activation_mut() {
            activation.after_statement = None;
        }
    }

    pub(crate) fn current_statement_result(&self) -> Result<Statement, InterpreterError> {
        self.current_statement()
            .ok_or(InterpreterError::NoCurrentStatement)
    }

    pub fn last_stop(&self) -> Option<&<M as crate::Machine<'ir>>::Stop> {
        self.last_stop.as_ref()
    }

    pub fn push_specialization(
        &mut self,
        callee: SpecializedFunction,
    ) -> Result<(), InterpreterError> {
        let entry = self.entry_block(callee)?;
        self.push_frame(callee, self.root_stage, entry.into(), None)
    }

    pub fn entry_block(&self, callee: SpecializedFunction) -> Result<Block, InterpreterError> {
        let stage = self.stage_info_for(self.active_stage());
        let spec_info = callee.expect_info(stage);
        let body = *spec_info.body();
        let region = body
            .regions(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)?;

        region
            .blocks(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)
    }

    pub(super) fn bind_block_args(
        &mut self,
        block: Block,
        args: impl IntoIterator<Item = V>,
    ) -> Result<(), E>
    where
        V: Clone,
        E: From<InterpreterError>,
        Self: ValueStore<Value = V, Error = E>,
    {
        let stage = self.stage_info_for(self.active_stage());
        let block_info = block.expect_info(stage);
        let expected = block_info.arguments.len();

        let mut got = 0;
        for (argument, value) in block_info.arguments.iter().zip(args) {
            self.write(SSAValue::from(*argument), value)?;
            got += 1;
        }

        if got != expected {
            return Err(InterpreterError::ArityMismatch { expected, got }.into());
        }

        Ok(())
    }

    pub(super) fn lookup_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<Function, InterpreterError> {
        let stage = self.stage_info_for(stage_id);
        let target_name = stage
            .symbol_table()
            .resolve(target)
            .cloned()
            .unwrap_or_else(|| format!("{target:?}"));

        self.pipeline
            .resolve_function(stage, target)
            .ok_or(InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget { name: target_name },
            })
    }

    pub(super) fn lookup_staged(
        &self,
        function: Function,
        stage_id: CompileStage,
    ) -> Result<StagedFunction, InterpreterError> {
        let function_info =
            self.pipeline
                .function_info(function)
                .ok_or(InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: StageResolutionError::MissingFunction { function },
                })?;

        function_info
            .staged_function(stage_id)
            .ok_or(InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::MissingFunction { function },
            })
    }

    pub(super) fn select_unique_live_specialization(
        &self,
        staged_function: StagedFunction,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError> {
        let stage = self.stage_info_for(stage_id);
        let staged_info = staged_function.get_info(stage).ok_or_else(|| {
            InterpreterError::custom(std::io::Error::other(format!(
                "missing staged function info for {staged_function:?} at stage {stage_id:?}"
            )))
        })?;

        staged_info
            .unique_live_specialization()
            .map_err(|error| match error {
                kirin_ir::UniqueLiveSpecializationError::NoSpecialization => {
                    InterpreterError::StageResolution {
                        stage: stage_id,
                        kind: StageResolutionError::NoSpecialization { staged_function },
                    }
                }
                kirin_ir::UniqueLiveSpecializationError::Ambiguous { count } => {
                    InterpreterError::StageResolution {
                        stage: stage_id,
                        kind: StageResolutionError::AmbiguousSpecialization {
                            staged_function,
                            count,
                        },
                    }
                }
            })
    }

    pub(crate) fn resume_seed_after_current(&self) -> Result<InternalSeed, InterpreterError> {
        let stage = self.stage_info_for(self.active_stage());
        let statement = self.current_statement_result()?;
        let block = self
            .current_block()
            .ok_or(InterpreterError::InvalidControl(
                "current statement is not block-local",
            ))?;
        let next = (*statement.next(stage)).or_else(|| block.terminator(stage));

        Ok(match next {
            Some(statement) => InternalBlockSeed::at_statement(block, statement).into(),
            None => InternalBlockSeed::exhausted(block).into(),
        })
    }

    pub fn start_specialization(&mut self, callee: SpecializedFunction, args: &[V]) -> Result<(), E>
    where
        V: Clone,
        E: From<InterpreterError>,
        Self: ValueStore<Value = V, Error = E>,
    {
        self.clear_frames();
        self.last_stop = None;
        self.skip_finish_step = false;

        let entry = self.entry_block(callee)?;
        self.push_frame(callee, self.root_stage, entry.into(), None)
            .map_err(E::from)?;

        if let Err(error) = self.bind_block_args(entry, args.iter().cloned()) {
            self.clear_frames();
            return Err(error);
        }

        Ok(())
    }

    pub fn apply_control(
        &mut self,
        control: Directive<<M as crate::Machine<'ir>>::Stop, BlockSeed<V>>,
    ) -> Result<(), E>
    where
        V: Clone,
        E: From<InterpreterError>,
        Self: ValueStore<Value = V, Error = E>,
    {
        match control {
            Directive::Advance => {
                let stage = self.active_stage();
                let stage = self.stage_info_for(stage);
                let activation = self.current_activation_mut().map_err(E::from)?;
                activation.after_statement = None;
                let cursor = activation
                    .cursor_stack
                    .last_mut()
                    .ok_or(InterpreterError::InvalidControl(
                        "advance requires an active cursor",
                    ))
                    .map_err(E::from)?;
                cursor.advance(stage);
                Ok(())
            }
            Directive::Stay => Ok(()),
            Directive::Push(seed) => {
                let (block, args) = seed.into_parts();
                let stage = self.active_stage();
                let internal_seed: InternalSeed = block.into();
                let next = ExecutionCursor::from_seed(self.stage_info_for(stage), internal_seed);
                let activation = self.current_activation_mut().map_err(E::from)?;
                activation.after_statement = None;
                activation.cursor_stack.push(next);
                self.bind_block_args(block, args)?;
                Ok(())
            }
            Directive::Replace(seed) => {
                let (block, args) = seed.into_parts();
                self.replace_current_seed(block.into()).map_err(E::from)?;
                self.bind_block_args(block, args)?;
                Ok(())
            }
            Directive::Pop => {
                let activation = self.current_activation_mut().map_err(E::from)?;
                activation.after_statement = None;
                activation
                    .cursor_stack
                    .pop()
                    .map(|_| ())
                    .ok_or(InterpreterError::InvalidControl(
                        "pop requires an active cursor",
                    ))
                    .map_err(E::from)
            }
            Directive::Stop(stop) => {
                self.last_stop = Some(stop);
                self.clear_frames();
                self.skip_finish_step = false;
                Ok(())
            }
        }
    }

    pub(super) fn prepare_invoke(
        &self,
        results: &[ResultValue],
    ) -> Result<Continuation, InterpreterError> {
        let completed_statement = self.current_statement_result()?;
        let resume = self.resume_seed_after_current()?;
        Ok(Continuation::new(
            completed_statement,
            resume,
            results.to_vec(),
        ))
    }

    pub(super) fn restore_caller(
        &mut self,
        continuation: &Continuation,
    ) -> Result<(), InterpreterError> {
        self.replace_current_seed(continuation.resume())?;
        let activation = self.current_activation_mut()?;
        activation.after_statement = Some(continuation.completed_statement());
        Ok(())
    }

    pub(super) fn write_return_product(
        &mut self,
        results: &[ResultValue],
        value: V,
    ) -> Result<(), E>
    where
        V: Clone + ProductValue,
        E: From<InterpreterError>,
        Self: ValueStore<Value = V, Error = E>,
    {
        match results {
            [] => Ok(()),
            [result] => self.write(*result, value),
            many => {
                let mut unpacked = Vec::with_capacity(many.len());
                for index in 0..many.len() {
                    unpacked.push(ProductValue::get(&value, index).map_err(E::from)?);
                }
                self.write_many(many, &unpacked)
            }
        }
    }

    pub fn run_specialization(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<crate::result::Run<<M as crate::Machine<'ir>>::Stop>, E>
    where
        V: Clone,
        M: crate::ConsumeEffect<'ir, Error = E> + crate::Machine<'ir, Seed = BlockSeed<V>>,
        L: crate::Interpretable<'ir, Self, Effect = <M as crate::Machine<'ir>>::Effect, Error = E>,
        E: From<InterpreterError>,
    {
        self.start_specialization(callee, args)?;
        <Self as crate::interpreter::Driver<'ir>>::run(self)
    }
}
