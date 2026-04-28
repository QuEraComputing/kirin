use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo};

use crate::{Env, EnvIndex, Frame, FrameEffect, InterpreterError, StepResult};

use super::{AbstractEnvStore, AbstractValue};

pub struct AbstractInterpreter<'ir, S, F, C, E, V> {
    pipeline: &'ir Pipeline<S>,
    frames: Vec<F>,
    envs: AbstractEnvStore<V>,
    _marker: PhantomData<(C, E)>,
}

impl<'ir, S, F, C, E, V> AbstractInterpreter<'ir, S, F, C, E, V> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            frames: Vec::new(),
            envs: AbstractEnvStore::new(),
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    pub fn push_frame(&mut self, frame: F) {
        self.frames.push(frame);
    }

    pub fn frame_depth(&self) -> usize {
        self.frames.len()
    }

    pub fn push_env(&mut self) -> EnvIndex {
        self.envs.push()
    }

    pub fn pop_env(&mut self) -> Result<EnvIndex, E>
    where
        E: From<InterpreterError>,
    {
        self.envs.pop().map_err(E::from)
    }

    pub fn current_env(&self) -> Result<EnvIndex, E>
    where
        E: From<InterpreterError>,
    {
        self.envs.current().map_err(E::from)
    }

    pub fn clone_env(&mut self, index: EnvIndex) -> Result<EnvIndex, E>
    where
        E: From<InterpreterError>,
        V: Clone,
    {
        self.envs.clone_store_from(index).map_err(E::from)
    }

    pub fn run(&mut self) -> Result<C, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        loop {
            match self.step()? {
                StepResult::Running => {}
                StepResult::Complete(completion) => return Ok(completion),
            }
        }
    }

    pub fn run_frame(&mut self, root: F) -> Result<C, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        let mut stack = vec![root];

        loop {
            let frame = stack.pop().ok_or(InterpreterError::EmptyFrameStack)?;
            let effect = frame.step(self)?;
            if let Some(completion) = self.apply_local_effect(&mut stack, effect)? {
                return Ok(completion);
            }
        }
    }

    pub fn step(&mut self) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        let frame = self.frames.pop().ok_or(InterpreterError::EmptyFrameStack)?;
        let effect = frame.step(self)?;
        self.apply_effect(effect)
    }

    fn apply_effect(&mut self, effect: FrameEffect<F, C>) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        match effect {
            FrameEffect::Continue(frame) => {
                self.frames.push(frame);
                Ok(StepResult::Running)
            }
            FrameEffect::Push { parent, child } => {
                self.frames.push(parent);
                self.frames.push(child);
                Ok(StepResult::Running)
            }
            FrameEffect::Done => {
                if let Some(parent) = self.frames.pop() {
                    let effect = parent.resume_done(self)?;
                    self.apply_effect(effect)
                } else {
                    Err(InterpreterError::EmptyFrameStack.into())
                }
            }
            FrameEffect::Complete(completion) => {
                if let Some(parent) = self.frames.pop() {
                    let effect = parent.resume(completion, self)?;
                    self.apply_effect(effect)
                } else {
                    Ok(StepResult::Complete(completion))
                }
            }
        }
    }

    fn apply_local_effect(
        &mut self,
        stack: &mut Vec<F>,
        effect: FrameEffect<F, C>,
    ) -> Result<Option<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        match effect {
            FrameEffect::Continue(frame) => {
                stack.push(frame);
                Ok(None)
            }
            FrameEffect::Push { parent, child } => {
                stack.push(parent);
                stack.push(child);
                Ok(None)
            }
            FrameEffect::Done => {
                if let Some(parent) = stack.pop() {
                    let effect = parent.resume_done(self)?;
                    self.apply_local_effect(stack, effect)
                } else {
                    Err(InterpreterError::EmptyFrameStack.into())
                }
            }
            FrameEffect::Complete(completion) => {
                if let Some(parent) = stack.pop() {
                    let effect = parent.resume(completion, self)?;
                    self.apply_local_effect(stack, effect)
                } else {
                    Ok(Some(completion))
                }
            }
        }
    }
}

impl<'ir, S, F, C, E, V> Env<V> for AbstractInterpreter<'ir, S, F, C, E, V>
where
    V: AbstractValue,
    E: From<InterpreterError>,
{
    type Error = E;

    fn alloc(&mut self) -> EnvIndex {
        self.envs.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.envs.free(index).map_err(E::from)
    }

    fn read(&self, index: EnvIndex, value: kirin_ir::SSAValue) -> Result<V, Self::Error> {
        self.envs.read(index, value).map_err(E::from)
    }

    fn write(
        &mut self,
        index: EnvIndex,
        value: kirin_ir::SSAValue,
        data: V,
    ) -> Result<(), Self::Error> {
        self.envs.write(index, value, data).map_err(E::from)
    }
}

impl<'ir, S, F, C, E, V, L> crate::StageAccess<L> for AbstractInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: From<InterpreterError>,
{
    type Error = E;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(E::from)
    }
}
