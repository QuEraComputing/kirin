use std::ops::ControlFlow;

use kirin_ir::{
    Block, CompileStage, Dialect, Function, GetInfo, HasStageInfo, Pipeline, Product, Region,
    ResultValue, SSAValue, SpecializedFunction, StageInfo,
};
use smallvec::SmallVec;

use crate::{
    Effect, InterpError, Interpretable, Interpreter, InterpreterError, Machine, PipelineAccess,
    ProductValue as RuntimeProductValue, ResolutionPolicy, StageResolutionError, ValueRead,
};

use super::cursor::ExecutionCursor;
use super::frame::Frame;
use super::frame_stack::FrameStack;

pub struct SingleStage<'ir, L, V, M, S = StageInfo<L>>
where
    L: Dialect,
    V: Clone + RuntimeProductValue,
    M: Machine,
    S: HasStageInfo<L>,
{
    pipeline: &'ir Pipeline<S>,
    current_stage: CompileStage,
    frames: FrameStack<V>,
    dialect_machine: M,
    result: Option<V>,
    _dialect: std::marker::PhantomData<fn() -> L>,
}

impl<'ir, L, V, M, S> SingleStage<'ir, L, V, M, S>
where
    L: Dialect,
    V: Clone + RuntimeProductValue,
    M: Machine,
    S: HasStageInfo<L>,
{
    #[must_use]
    pub fn new(pipeline: &'ir Pipeline<S>, stage: CompileStage, dialect_machine: M) -> Self {
        Self {
            pipeline,
            current_stage: stage,
            frames: FrameStack::new(),
            dialect_machine,
            result: None,
            _dialect: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn machine(&self) -> &M {
        &self.dialect_machine
    }

    pub fn machine_mut(&mut self) -> &mut M {
        &mut self.dialect_machine
    }

    pub(crate) fn stage_info(&self) -> Result<&'ir StageInfo<L>, InterpreterError> {
        let stage =
            self.pipeline
                .stage(self.current_stage)
                .ok_or(InterpreterError::StageResolution {
                    stage: self.current_stage,
                    kind: StageResolutionError::MissingStage,
                })?;

        stage
            .try_stage_info()
            .ok_or(InterpreterError::StageResolution {
                stage: self.current_stage,
                kind: StageResolutionError::TypeMismatch,
            })
    }

    pub(crate) fn current_statement(&self) -> Result<kirin_ir::Statement, InterpreterError> {
        self.frames
            .current()?
            .cursor()
            .statement()
            .ok_or(InterpreterError::NoCurrentStatement)
    }

    pub(crate) fn current_cursor(&self) -> Result<ExecutionCursor, InterpreterError> {
        Ok(*self.frames.current()?.cursor())
    }

    pub(crate) fn set_cursor(&mut self, cursor: ExecutionCursor) -> Result<(), InterpreterError> {
        *self.frames.current_mut()?.cursor_mut() = cursor;
        Ok(())
    }

    fn entry_region(&self, callee: SpecializedFunction) -> Result<Region, InterpreterError> {
        let stage = self.stage_info()?;
        let body = *callee.expect_info(stage).body();

        body.regions(stage)
            .next()
            .copied()
            .ok_or_else(InterpreterError::missing_function_entry)
    }

    pub(crate) fn region_entry_block(&self, region: Region) -> Result<Block, InterpreterError> {
        let stage = self.stage_info()?;
        region
            .blocks(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)
    }

    fn entry_block(&self, callee: SpecializedFunction) -> Result<Block, InterpreterError> {
        let region = self.entry_region(callee)?;
        self.region_entry_block(region)
    }

    pub(crate) fn specialization_entry_region(
        &self,
        callee: SpecializedFunction,
    ) -> Result<Region, InterpreterError> {
        self.entry_region(callee)
    }

    pub(crate) fn bind_block_args(
        &mut self,
        block: Block,
        args: impl IntoIterator<Item = V>,
    ) -> Result<(), InterpreterError> {
        let stage = self.stage_info()?;
        let block_info = block.expect_info(stage);
        let args: Vec<V> = args.into_iter().collect();
        let expected = block_info.arguments.len();

        if args.len() != expected {
            return Err(InterpreterError::ArityMismatch {
                expected,
                got: args.len(),
            });
        }

        let frame = self.frames.current_mut()?;
        for (argument, value) in block_info.arguments.iter().zip(args) {
            frame.write_ssa((*argument).into(), value);
        }

        Ok(())
    }

    pub(crate) fn current_effect(&mut self) -> Result<Effect<V, M::Effect>, InterpError<M::Error>>
    where
        L: Interpretable<Self>,
        M::Effect: crate::Lift<L::Effect>,
        M::Error: crate::Lift<L::Error>,
    {
        let statement = self.current_statement()?;
        let stage = self.stage_info()?;
        let effect = statement
            .definition(stage)
            .interpret(self)
            .map_err(lift_interp_error::<L::Error, M::Error>)?;

        Ok(lift_effect::<V, L::Effect, M::Effect>(effect))
    }

    pub(crate) fn enter_block(
        &mut self,
        block: Block,
        args: SmallVec<[V; 2]>,
    ) -> Result<(), InterpreterError> {
        let stage = self.stage_info()?;
        let block_info = block.expect_info(stage);
        let args: Vec<V> = args.into_iter().collect();
        let expected = block_info.arguments.len();

        if args.len() != expected {
            return Err(InterpreterError::ArityMismatch {
                expected,
                got: args.len(),
            });
        }

        let frame = self.frames.current_mut()?;
        frame.cursor_mut().jump_to(stage, block);
        for (argument, value) in block_info.arguments.iter().zip(args) {
            frame.write_ssa((*argument).into(), value);
        }

        Ok(())
    }

    pub(crate) fn push_specialization_frame(
        &mut self,
        callee: SpecializedFunction,
    ) -> Result<(), InterpreterError> {
        let entry = self.entry_block(callee)?;
        let cursor = ExecutionCursor::entry(self.stage_info()?, entry);
        self.frames.push(Frame::new(cursor, None));
        Ok(())
    }

    pub(crate) fn pop_current_frame(&mut self) -> Result<(), InterpreterError> {
        self.frames.pop().map(|_| ())
    }

    pub(crate) fn clear_result(&mut self) {
        self.result = None;
    }

    fn bind_product(
        &mut self,
        results: Product<ResultValue>,
        value: V,
    ) -> Result<(), InterpreterError> {
        match results.len() {
            0 => Ok(()),
            1 => {
                let result = results[0];
                self.frames
                    .current_mut()?
                    .write_ssa(SSAValue::from(result), value);
                Ok(())
            }
            _ => {
                let frame = self.frames.current_mut()?;
                for (index, result) in results.iter().enumerate() {
                    frame.write_ssa(SSAValue::from(*result), value.get(index)?);
                }
                Ok(())
            }
        }
    }

    fn advance_cursor(&mut self) -> Result<(), InterpreterError> {
        let stage = self.stage_info()?;
        self.frames.current_mut()?.cursor_mut().advance(stage);
        Ok(())
    }

    fn jump_to(&mut self, block: Block, args: SmallVec<[V; 2]>) -> Result<(), InterpreterError> {
        self.enter_block(block, args)
    }

    fn pop_frame_with(&mut self, value: V) -> Result<(), InterpreterError> {
        let frame = self.frames.pop()?;

        if let Some(continuation) = frame.continuation() {
            if self.frames.is_empty() {
                return Err(InterpreterError::InvalidControl(
                    "return continuation missing caller frame",
                ));
            }

            let caller = self.frames.current_mut()?;
            *caller.cursor_mut() = continuation.resume();
            for (index, result) in continuation.results().iter().enumerate() {
                let element = if continuation.results().len() == 1 {
                    value.clone()
                } else {
                    value.get(index)?
                };
                caller.write_ssa(SSAValue::from(*result), element);
            }

            Ok(())
        } else {
            self.result = Some(value);
            Ok(())
        }
    }

    fn yield_to_caller(&mut self, value: V) {
        self.result = Some(value);
    }

    pub fn start_specialization(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<(), InterpreterError> {
        self.frames.clear();
        self.result = None;

        let entry = self.entry_block(callee)?;
        self.push_specialization_frame(callee)?;
        self.bind_block_args(entry, args.iter().cloned())
    }
}

impl<'ir, L, V, M, S> Machine for SingleStage<'ir, L, V, M, S>
where
    L: Dialect,
    V: Clone + RuntimeProductValue,
    M: Machine,
    S: HasStageInfo<L>,
{
    type Effect = Effect<V, M::Effect>;
    type Error = InterpError<M::Error>;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error> {
        match effect {
            Effect::Advance => self.advance_cursor().map_err(Into::into),
            Effect::Stay => Ok(()),
            Effect::Jump(block, args) => self.jump_to(block, args).map_err(Into::into),
            Effect::BindValue(ssa, value) => {
                self.frames.current_mut()?.write_ssa(ssa, value);
                Ok(())
            }
            Effect::BindProduct(results, value) => {
                self.bind_product(results, value).map_err(Into::into)
            }
            Effect::Return(value) => self.pop_frame_with(value).map_err(Into::into),
            Effect::Yield(value) => {
                self.yield_to_caller(value);
                Ok(())
            }
            Effect::Stop(value) => {
                self.result = Some(value);
                Ok(())
            }
            Effect::Seq(effects) => {
                for effect in effects {
                    self.consume_effect(*effect)?;
                }
                Ok(())
            }
            Effect::Machine(effect) => self
                .dialect_machine
                .consume_effect(effect)
                .map_err(InterpError::Dialect),
        }
    }
}

impl<'ir, L, V, M, S> ValueRead for SingleStage<'ir, L, V, M, S>
where
    L: Dialect,
    V: Clone + RuntimeProductValue,
    M: Machine,
    S: HasStageInfo<L>,
{
    type Value = V;

    fn read(&self, value: SSAValue) -> Result<Self::Value, InterpreterError> {
        self.frames
            .current()?
            .read(value)
            .cloned()
            .ok_or(InterpreterError::UnboundValue(value))
    }
}

impl<'ir, L, V, M, S> PipelineAccess for SingleStage<'ir, L, V, M, S>
where
    L: Dialect,
    V: Clone + RuntimeProductValue,
    M: Machine,
    S: HasStageInfo<L>,
{
    type StageInfo = S;

    fn pipeline(&self) -> &Pipeline<Self::StageInfo> {
        self.pipeline
    }

    fn current_stage(&self) -> CompileStage {
        self.current_stage
    }

    fn resolve_callee(
        &self,
        function: Function,
        _args: &[<Self as ValueRead>::Value],
        policy: ResolutionPolicy,
    ) -> Result<SpecializedFunction, InterpreterError> {
        match policy {
            ResolutionPolicy::UniqueLive => {
                let function_info = self.pipeline.function_info(function).ok_or(
                    InterpreterError::StageResolution {
                        stage: self.current_stage,
                        kind: StageResolutionError::MissingFunction { function },
                    },
                )?;
                let staged_function = function_info.staged_function(self.current_stage).ok_or(
                    InterpreterError::StageResolution {
                        stage: self.current_stage,
                        kind: StageResolutionError::MissingFunction { function },
                    },
                )?;
                let stage = self.stage_info()?;
                let staged_info =
                    staged_function
                        .get_info(stage)
                        .ok_or(InterpreterError::StageResolution {
                            stage: self.current_stage,
                            kind: StageResolutionError::MissingFunction { function },
                        })?;

                staged_info
                    .unique_live_specialization()
                    .map_err(|error| match error {
                        kirin_ir::UniqueLiveSpecializationError::NoSpecialization => {
                            InterpreterError::StageResolution {
                                stage: self.current_stage,
                                kind: StageResolutionError::NoSpecialization { staged_function },
                            }
                        }
                        kirin_ir::UniqueLiveSpecializationError::Ambiguous { count } => {
                            InterpreterError::StageResolution {
                                stage: self.current_stage,
                                kind: StageResolutionError::AmbiguousSpecialization {
                                    staged_function,
                                    count,
                                },
                            }
                        }
                    })
            }
        }
    }
}

impl<'ir, L, V, M, S> Interpreter for SingleStage<'ir, L, V, M, S>
where
    L: Dialect + Interpretable<Self>,
    V: Clone + RuntimeProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: HasStageInfo<L>,
{
    type Dialect = L;
    type DialectEffect = M::Effect;
    type DialectError = M::Error;

    fn step(&mut self) -> Result<ControlFlow<Self::Value>, Self::Error> {
        let effect = self.current_effect()?;
        self.consume_effect(effect)?;

        if let Some(value) = self.result.take() {
            Ok(ControlFlow::Break(value))
        } else {
            Ok(ControlFlow::Continue(()))
        }
    }
}

fn lift_effect<V, From, To>(effect: Effect<V, From>) -> Effect<V, To>
where
    To: crate::Lift<From>,
{
    match effect {
        Effect::Advance => Effect::Advance,
        Effect::Stay => Effect::Stay,
        Effect::Jump(block, args) => Effect::Jump(block, args),
        Effect::BindValue(ssa, value) => Effect::BindValue(ssa, value),
        Effect::BindProduct(results, value) => Effect::BindProduct(results, value),
        Effect::Return(value) => Effect::Return(value),
        Effect::Yield(value) => Effect::Yield(value),
        Effect::Stop(value) => Effect::Stop(value),
        Effect::Seq(effects) => Effect::Seq(
            effects
                .into_iter()
                .map(|effect| Box::new(lift_effect(*effect)))
                .collect(),
        ),
        Effect::Machine(effect) => Effect::Machine(To::lift(effect)),
    }
}

fn lift_interp_error<From, To>(error: InterpError<From>) -> InterpError<To>
where
    To: crate::Lift<From>,
{
    match error {
        InterpError::Interpreter(error) => InterpError::Interpreter(error),
        InterpError::Dialect(error) => InterpError::Dialect(To::lift(error)),
    }
}
