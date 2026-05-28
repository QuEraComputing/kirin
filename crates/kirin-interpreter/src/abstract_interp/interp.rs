use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo};

use crate::{
    Env, EnvIndex, ForkEnv, Frame, FrameEffect, FunctionInvocation, FunctionInvokeBuilder,
    InterpreterError, InterpreterProfile, StageFrame, StepResult,
};

use super::AbstractEnvStore;

pub type AbstractInterpreter<'ir, P> =
    AbstractInterpreterWithStore<'ir, P, AbstractEnvStore<<P as InterpreterProfile>::Value>>;

pub struct AbstractInterpreterWithStore<'ir, P: InterpreterProfile, Store> {
    pipeline: &'ir Pipeline<P::Stage>,
    frames: Vec<P::Frame>,
    store: Store,
    _marker: PhantomData<P>,
}

impl<'ir, P: InterpreterProfile, Store> AbstractInterpreterWithStore<'ir, P, Store> {
    pub fn with_store(pipeline: &'ir Pipeline<P::Stage>, store: Store) -> Self {
        Self {
            pipeline,
            frames: Vec::new(),
            store,
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<P::Stage> {
        self.pipeline
    }

    pub fn push_frame(&mut self, frame: P::Frame) {
        self.frames.push(frame);
    }

    pub fn frame_depth(&self) -> usize {
        self.frames.len()
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    pub fn invoke(&mut self, stage: CompileStage) -> FunctionInvokeBuilder<'_, Self> {
        FunctionInvokeBuilder::new(self, stage)
    }

    pub fn run(&mut self) -> Result<P::Completion, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
        loop {
            match self.step()? {
                StepResult::Running => {}
                StepResult::Complete(completion) => return Ok(completion),
            }
        }
    }

    pub fn run_function_invocation(
        &mut self,
        invocation: FunctionInvocation<P::Value>,
    ) -> Result<P::Completion, P::Error>
    where
        Store: Env<P::Value>,
        P::Frame: StageFrame<P::Stage, P::Value> + Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error:
            From<InterpreterError> + From<<P::Frame as StageFrame<P::Stage, P::Value>>::Error>,
    {
        let stage = invocation.stage();
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        let frame =
            P::Frame::from_function_invocation(stage_info, invocation).map_err(P::Error::from)?;
        self.push_frame(frame);
        self.run()
    }

    /// Resolve a function by name in the given stage and run it with the
    /// given arguments. Returns the raw completion produced by the function.
    pub fn run_function_by_name<A>(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: A,
    ) -> Result<P::Completion, P::Error>
    where
        P::Stage: kirin_ir::StageMeta,
        Store: Env<P::Value>,
        Self: crate::FrameDispatch<P::Frame, P::Value, P::Error>,
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
        A: IntoIterator<Item = P::Value>,
    {
        let stage = self
            .pipeline
            .stage_by_name(stage_name)
            .ok_or_else(|| InterpreterError::MissingStageName(stage_name.into()))
            .map_err(P::Error::from)?;
        let function = self
            .pipeline
            .lookup_function_by_name(function_name)
            .ok_or_else(|| InterpreterError::MissingFunctionName(function_name.into()))
            .map_err(P::Error::from)?;
        self.invoke(stage).function(function).args(args)
    }

    pub fn run_frame(&mut self, root: P::Frame) -> Result<P::Completion, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
        let mut stack = vec![root];

        loop {
            let frame = match stack.pop() {
                Some(frame) => frame,
                None => return Err(P::Error::from(InterpreterError::EmptyFrameStack)),
            };
            let effect = frame.step(self)?;
            if let Some(completion) = self.apply_local_effect(&mut stack, effect)? {
                return Ok(completion);
            }
        }
    }

    pub fn step(&mut self) -> Result<StepResult<P::Completion>, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
        let frame = match self.frames.pop() {
            Some(frame) => frame,
            None => return Err(P::Error::from(InterpreterError::EmptyFrameStack)),
        };
        let effect = frame.step(self)?;
        self.apply_effect(effect)
    }

    fn apply_effect(
        &mut self,
        mut effect: FrameEffect<P::Frame, P::Completion>,
    ) -> Result<StepResult<P::Completion>, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
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
                    let parent = match self.frames.pop() {
                        Some(parent) => parent,
                        None => return Err(P::Error::from(InterpreterError::EmptyFrameStack)),
                    };
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

    fn apply_local_effect(
        &mut self,
        stack: &mut Vec<P::Frame>,
        mut effect: FrameEffect<P::Frame, P::Completion>,
    ) -> Result<Option<P::Completion>, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
        loop {
            match effect {
                FrameEffect::Continue(frame) => {
                    stack.push(frame);
                    return Ok(None);
                }
                FrameEffect::Push { parent, child } => {
                    stack.push(parent);
                    stack.push(child);
                    return Ok(None);
                }
                FrameEffect::Done => {
                    let parent = match stack.pop() {
                        Some(parent) => parent,
                        None => return Err(P::Error::from(InterpreterError::EmptyFrameStack)),
                    };
                    effect = parent.resume_done(self)?;
                }
                FrameEffect::Complete(completion) => {
                    if let Some(parent) = stack.pop() {
                        effect = parent.resume(completion, self)?;
                    } else {
                        return Ok(Some(completion));
                    }
                }
            }
        }
    }
}

impl<'ir, P: InterpreterProfile> AbstractInterpreter<'ir, P> {
    pub fn new(pipeline: &'ir Pipeline<P::Stage>) -> Self {
        Self::with_store(pipeline, AbstractEnvStore::new())
    }

    pub fn push_env(&mut self) -> EnvIndex {
        self.store.push()
    }

    pub fn pop_env(&mut self) -> Result<EnvIndex, P::Error>
    where
        P::Error: From<InterpreterError>,
    {
        self.store.pop().map_err(P::Error::from)
    }

    pub fn current_env(&self) -> Result<EnvIndex, P::Error>
    where
        P::Error: From<InterpreterError>,
    {
        self.store.current().map_err(P::Error::from)
    }

    pub fn clone_env(&mut self, index: EnvIndex) -> Result<EnvIndex, P::Error>
    where
        P::Error: From<InterpreterError>,
        P::Value: Clone,
    {
        self.store.clone_store_from(index).map_err(P::Error::from)
    }
}

impl<'ir, P, Store> Env<P::Value> for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    Store: Env<P::Value>,
    P::Error: From<Store::Error>,
{
    type Error = P::Error;

    fn alloc(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.store.free(index).map_err(P::Error::from)
    }

    fn read(&self, index: EnvIndex, value: kirin_ir::SSAValue) -> Result<P::Value, Self::Error> {
        self.store.read(index, value).map_err(P::Error::from)
    }

    fn write(
        &mut self,
        index: EnvIndex,
        value: kirin_ir::SSAValue,
        data: P::Value,
    ) -> Result<(), Self::Error> {
        self.store.write(index, value, data).map_err(P::Error::from)
    }
}

impl<'ir, P, Store> ForkEnv<P::Value> for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    Store: ForkEnv<P::Value>,
    P::Error: From<Store::Error>,
{
    fn fork_env(&mut self, index: EnvIndex) -> Result<EnvIndex, Self::Error> {
        self.store.fork_env(index).map_err(P::Error::from)
    }
}

impl<'ir, P, Store, L> crate::StageAccess<L> for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or_else(|| P::Error::from(InterpreterError::MissingStage(stage)))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(P::Error::from)
    }
}
