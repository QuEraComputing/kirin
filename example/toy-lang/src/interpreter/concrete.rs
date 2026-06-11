//! Cross-language concrete interpreter.
//!
//! [`ToyConcreteInterpreter`] is structurally a clone of
//! [`kirin_interpreter::ConcreteInterpreter`] specialised to
//! `ToyStageFrame<i64>`. The duplication exists for a single reason:
//! `kirin-function` already supplies a blanket `CallTargetResolution<L>` impl
//! for [`ConcreteInterpreter`](kirin_interpreter::ConcreteInterpreter) that
//! resolves calls within the caller's stage only, and Rust does not permit
//! specialization. A distinct struct lets us attach our own cross-stage
//! [`CallTargetResolution`] impl (via
//! [`super::cross_stage::resolve_cross_stage_call_target`]) so that a
//! source-stage function can call a function whose body lives only at the
//! lowered stage (and vice versa).
//!
//! The single-stage runners [`run_source_i64`](super::run_source_i64) and
//! [`run_lowered_i64`](super::run_lowered_i64) keep using the framework's
//! [`ConcreteInterpreter`](kirin_interpreter::ConcreteInterpreter) directly —
//! they retain the default same-stage resolution.

use kirin::prelude::{
    Block, CompileStage, CompileTimeValue, Dialect, Function, GetInfo, HasStageInfo, Pipeline,
    Product, Region, ResultValue, SSAValue, SpecializedFunction, StageInfo, StagedFunction,
    Statement, Symbol, UniqueLiveSpecializationError,
};
use kirin_function::interpreter::{
    CallTargetResolution, FunctionRegionDispatch, ResolvedCallTarget,
};
use kirin_interpreter::{
    AbstractBranchFrame, BlockFrame, BlockTransferDispatch, ConcreteBlockTransfer, Env, EnvIndex,
    EnvStackStore, Frame, FrameDispatch, FrameEffect, FunctionAccess, FunctionBodyDispatch,
    FunctionEntry, FunctionInvocation, InterpreterError, Location, Position, RegionFrame,
    StageAccess, StageFrame, StandardFrame, StepResult, expect_single_function_return,
};
use kirin_scf::interpreter::{ForContinuation, ScfBlockDispatch, ScfForDispatch, ScfIfDispatch};

use crate::stage::Stage;

use super::cross_stage::resolve_cross_stage_call_target;
use super::{ToyCompletion, ToyError, ToyStageFrame};

pub struct ToyConcreteInterpreter<'ir> {
    pipeline: &'ir Pipeline<Stage>,
    frames: Vec<ToyStageFrame<i64>>,
    envs: EnvStackStore<i64>,
}

impl<'ir> ToyConcreteInterpreter<'ir> {
    pub fn new(pipeline: &'ir Pipeline<Stage>) -> Self {
        Self {
            pipeline,
            frames: Vec::new(),
            envs: EnvStackStore::new(),
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<Stage> {
        self.pipeline
    }

    pub fn run_function_by_name<A>(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: A,
    ) -> Result<ToyCompletion<i64>, ToyError>
    where
        A: IntoIterator<Item = i64>,
    {
        let stage = self
            .pipeline
            .stage_by_name(stage_name)
            .ok_or_else(|| InterpreterError::MissingStageName(stage_name.into()))
            .map_err(ToyError::from)?;
        let function = self
            .pipeline
            .lookup_function_by_name(function_name)
            .ok_or_else(|| InterpreterError::MissingFunctionName(function_name.into()))
            .map_err(ToyError::from)?;
        self.run_function_invocation(FunctionInvocation::function(
            stage,
            function,
            Product::from_iter(args),
        ))
    }

    pub fn run_function_invocation(
        &mut self,
        invocation: FunctionInvocation<i64>,
    ) -> Result<ToyCompletion<i64>, ToyError> {
        let stage = invocation.stage();
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))
            .map_err(ToyError::from)?;
        let frame = ToyStageFrame::<i64>::from_function_invocation(stage_info, invocation)
            .map_err(ToyError::from)?;
        self.frames.push(frame);
        self.run()
    }

    fn run(&mut self) -> Result<ToyCompletion<i64>, ToyError> {
        loop {
            match self.step()? {
                StepResult::Running => {}
                StepResult::Complete(completion) => return Ok(completion),
            }
        }
    }

    fn step(&mut self) -> Result<StepResult<ToyCompletion<i64>>, ToyError> {
        let frame = self
            .frames
            .pop()
            .ok_or(InterpreterError::EmptyFrameStack)
            .map_err(ToyError::from)?;
        let effect = <ToyStageFrame<i64> as Frame<
            Self,
            ToyStageFrame<i64>,
            ToyCompletion<i64>,
            ToyError,
        >>::step(frame, self)?;
        self.apply_effect(effect)
    }

    fn apply_effect(
        &mut self,
        mut effect: FrameEffect<ToyStageFrame<i64>, ToyCompletion<i64>>,
    ) -> Result<StepResult<ToyCompletion<i64>>, ToyError> {
        loop {
            match effect {
                FrameEffect::Continue(frame) => {
                    self.frames.push(frame);
                    return Ok(StepResult::Running);
                }
                FrameEffect::Push { parent, child } => {
                    self.frames.push(parent);
                    self.frames.push(child);
                    return Ok(StepResult::Running);
                }
                FrameEffect::Done => {
                    let parent = self
                        .frames
                        .pop()
                        .ok_or(InterpreterError::EmptyFrameStack)
                        .map_err(ToyError::from)?;
                    effect = parent.resume_done(self)?;
                }
                FrameEffect::Complete(completion) => {
                    if let Some(parent) = self.frames.pop() {
                        effect = parent.resume(completion, self)?;
                    } else {
                        return Ok(StepResult::Complete(completion));
                    }
                }
            }
        }
    }
}

impl Env<i64> for ToyConcreteInterpreter<'_> {
    type Error = ToyError;

    fn alloc(&mut self) -> EnvIndex {
        self.envs.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.envs.free(index).map_err(ToyError::from)
    }

    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<i64, Self::Error> {
        self.envs.read(index, value).map_err(ToyError::from)
    }

    fn write(&mut self, index: EnvIndex, value: SSAValue, data: i64) -> Result<(), Self::Error> {
        self.envs.write(index, value, data).map_err(ToyError::from)
    }
}

impl<L: Dialect> StageAccess<L> for ToyConcreteInterpreter<'_>
where
    Stage: HasStageInfo<L>,
{
    type Error = ToyError;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))
            .map_err(ToyError::from)?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(ToyError::from)
    }
}

impl FrameDispatch<ToyStageFrame<i64>, i64, ToyError> for ToyConcreteInterpreter<'_> {
    fn dispatch_function_invocation(
        &mut self,
        invocation: FunctionInvocation<i64>,
    ) -> Result<ToyStageFrame<i64>, ToyError> {
        let stage = invocation.stage();
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))
            .map_err(ToyError::from)?;
        ToyStageFrame::<i64>::from_function_invocation(stage_info, invocation)
            .map_err(ToyError::from)
    }

    fn dispatch_block(
        &mut self,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<i64>,
    ) -> Result<ToyStageFrame<i64>, ToyError> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))
            .map_err(ToyError::from)?;
        ToyStageFrame::<i64>::from_block(stage_info, stage, block, env, args)
            .map_err(ToyError::from)
    }
}

impl<L, F, C, E> BlockTransferDispatch<L, F, C, E, i64, ConcreteBlockTransfer<i64>>
    for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    F: TryFrom<StandardFrame<L, i64, ConcreteBlockTransfer<i64>>>,
    E: From<InterpreterError>
        + From<<F as TryFrom<StandardFrame<L, i64, ConcreteBlockTransfer<i64>>>>::Error>,
{
    fn dispatch_block_transfer(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        transfer: ConcreteBlockTransfer<i64>,
    ) -> Result<FrameEffect<F, C>, E> {
        match transfer {
            ConcreteBlockTransfer::Jump { target, arguments } => {
                StandardFrame::Block(BlockFrame::<L, i64, ConcreteBlockTransfer<i64>>::new(
                    stage, target, env, arguments,
                ))
                .try_into()
                .map(FrameEffect::Continue)
                .map_err(E::from)
            }
        }
    }
}

impl<L: Dialect> FunctionAccess<L> for ToyConcreteInterpreter<'_>
where
    Stage: HasStageInfo<L>,
{
    type Error = ToyError;

    fn staged_function(
        &self,
        stage: CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline
            .function_info(function)
            .ok_or(InterpreterError::MissingFunction(function))
            .map_err(ToyError::from)?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(ToyError::from)
    }

    fn specialized_function(
        &self,
        stage: CompileStage,
        function: StagedFunction,
    ) -> Result<SpecializedFunction, Self::Error> {
        let stage_info = <Self as StageAccess<L>>::stage_info(self, stage)?;
        // `expect_info` panics if `function` was not registered at `stage`.
        // The framework's `ConcreteInterpreter::specialized_function` uses
        // the same call and relies on the same invariant: callers reach
        // this method via `FunctionFrame::step`, which only produces
        // `StagedFunction` ids that came from `interp.staged_function(stage, _)`
        // — i.e. ids that are guaranteed to exist at `stage`.
        let info = function.expect_info(stage_info);
        match info.unique_live_specialization() {
            Ok(function) => Ok(function),
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(ToyError::from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(ToyError::from(InterpreterError::AmbiguousSpecialization {
                    function,
                    count,
                }))
            }
        }
    }

    fn function_body(
        &self,
        stage: CompileStage,
        function: SpecializedFunction,
    ) -> Result<Statement, Self::Error> {
        let stage_info = <Self as StageAccess<L>>::stage_info(self, stage)?;
        function
            .get_info(stage_info)
            .map(|info| *info.body())
            .ok_or(InterpreterError::MissingFunctionBody { function, stage })
            .map_err(ToyError::from)
    }
}

impl<L, F> FunctionBodyDispatch<L, F, ToyError, i64> for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    L: FunctionEntry<L, Self, F, ToyError, i64>,
    Stage: HasStageInfo<L>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: Statement,
        env: EnvIndex,
        args: Product<i64>,
    ) -> Result<F, ToyError> {
        let location = Location::new(location.stage, Position::Statement { statement: body });
        let definition = {
            let stage = <Self as StageAccess<L>>::stage_info(self, location.stage)?;
            body.definition(stage).clone()
        };
        definition.enter_function_body(location, env, self, args)
    }
}

impl<L, F> FunctionRegionDispatch<L, F, ToyError, i64> for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    F: TryFrom<StandardFrame<L, i64, ConcreteBlockTransfer<i64>>>,
    ToyError: From<<F as TryFrom<StandardFrame<L, i64, ConcreteBlockTransfer<i64>>>>::Error>,
{
    fn dispatch_function_region(
        &mut self,
        location: Location,
        region: Region,
        env: EnvIndex,
        args: Product<i64>,
    ) -> Result<F, ToyError> {
        StandardFrame::Region(RegionFrame::<L, i64, ConcreteBlockTransfer<i64>>::new(
            location.stage,
            region,
            env,
            args,
        ))
        .try_into()
        .map_err(ToyError::from)
    }
}

impl<L, F, E, V, T> ScfBlockDispatch<L, F, E, V, T> for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    F: TryFrom<StandardFrame<L, V, T>>,
    E: From<<F as TryFrom<StandardFrame<L, V, T>>>::Error>,
{
    fn dispatch_scf_block(
        &mut self,
        location: Location,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        StandardFrame::Block(BlockFrame::<L, V, T>::new(location.stage, block, env, args))
            .try_into()
            .map_err(E::from)
    }
}

impl<L, F, C, E, V> ScfIfDispatch<L, F, C, E, V> for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    E: From<InterpreterError>,
{
    fn dispatch_indeterminate_if(
        &mut self,
        _location: Location,
        _env: EnvIndex,
        _then_body: Block,
        _else_body: Block,
        _results: Vec<ResultValue>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::IndeterminateBranch))
    }
}

impl<L, T, F, C, E, V, X> ScfForDispatch<L, T, F, C, E, V, X> for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    T: CompileTimeValue,
    E: From<InterpreterError>,
{
    fn dispatch_indeterminate_for(
        &mut self,
        _continuation: ForContinuation<L, T, V, X>,
    ) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::IndeterminateBranch))
    }
}

// `kirin-interpreter` ships a `Frame` impl for `AbstractBranchFrame` that is
// gated by the sealed `AbstractBranchShell` trait, plus a no-op impl
// specifically for `ConcreteInterpreter`. Mirror that no-op impl here so
// `ToyConcreteInterpreter` satisfies the bounds required by
// `StandardFrame::Frame` — concrete execution cannot legitimately reach an
// abstract branch frame, so this is dead code at runtime.
impl<L, F, C, E, V> Frame<ToyConcreteInterpreter<'_>, F, C, E> for AbstractBranchFrame<L, V>
where
    E: From<InterpreterError>,
{
    fn step(self, _interp: &mut ToyConcreteInterpreter<'_>) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::Custom(
            "ToyConcreteInterpreter cannot run abstract branch frame",
        )))
    }

    fn resume_done(self, _interp: &mut ToyConcreteInterpreter<'_>) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::Custom(
            "ToyConcreteInterpreter cannot run abstract branch frame",
        )))
    }

    fn resume(
        self,
        completion: C,
        _interp: &mut ToyConcreteInterpreter<'_>,
    ) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}

impl<L> CallTargetResolution<L> for ToyConcreteInterpreter<'_>
where
    L: Dialect,
    Stage: HasStageInfo<L>,
{
    type Error = ToyError;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        resolve_cross_stage_call_target::<L>(self.pipeline, location, target)
    }
}

pub fn run_i64(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp = ToyConcreteInterpreter::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        stage_name,
        function_name,
        args.iter().copied(),
    )?)
}
