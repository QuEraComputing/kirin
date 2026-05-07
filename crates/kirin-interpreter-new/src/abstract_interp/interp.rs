use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, LiftFrom, Pipeline, StageInfo};

use crate::{Env, EnvIndex, ForkEnv, Frame, FrameEffect, InterpreterError, StepResult};

use super::AbstractEnvStore;

pub type AbstractInterpreter<'ir, S, F, C, E, V> =
    AbstractInterpreterWithStore<'ir, S, F, C, E, AbstractEnvStore<V>>;

pub struct AbstractInterpreterWithStore<'ir, S, F, C, E, Store> {
    pipeline: &'ir Pipeline<S>,
    frames: Vec<F>,
    store: Store,
    _marker: PhantomData<(C, E)>,
}

impl<'ir, S, F, C, E, Store> AbstractInterpreterWithStore<'ir, S, F, C, E, Store> {
    pub fn with_store(pipeline: &'ir Pipeline<S>, store: Store) -> Self {
        Self {
            pipeline,
            frames: Vec::new(),
            store,
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

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    pub fn run(&mut self) -> Result<C, E>
    where
        F: Frame<Self, F, C, E>,
        E: LiftFrom<InterpreterError>,
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
        E: LiftFrom<InterpreterError>,
    {
        let mut stack = vec![root];

        loop {
            let frame = match stack.pop() {
                Some(frame) => frame,
                None => return Err(E::lift_from(InterpreterError::EmptyFrameStack)),
            };
            let effect = frame.step(self)?;
            if let Some(completion) = self.apply_local_effect(&mut stack, effect)? {
                return Ok(completion);
            }
        }
    }

    pub fn step(&mut self) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: LiftFrom<InterpreterError>,
    {
        let frame = match self.frames.pop() {
            Some(frame) => frame,
            None => return Err(E::lift_from(InterpreterError::EmptyFrameStack)),
        };
        let effect = frame.step(self)?;
        self.apply_effect(effect)
    }

    fn apply_effect(&mut self, mut effect: FrameEffect<F, C>) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: LiftFrom<InterpreterError>,
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
                        None => return Err(E::lift_from(InterpreterError::EmptyFrameStack)),
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
        stack: &mut Vec<F>,
        mut effect: FrameEffect<F, C>,
    ) -> Result<Option<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: LiftFrom<InterpreterError>,
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
                        None => return Err(E::lift_from(InterpreterError::EmptyFrameStack)),
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

impl<'ir, S, F, C, E, V> AbstractInterpreter<'ir, S, F, C, E, V> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self::with_store(pipeline, AbstractEnvStore::new())
    }

    pub fn push_env(&mut self) -> EnvIndex {
        self.store.push()
    }

    pub fn pop_env(&mut self) -> Result<EnvIndex, E>
    where
        E: LiftFrom<InterpreterError>,
    {
        self.store.pop().map_err(E::lift_from)
    }

    pub fn current_env(&self) -> Result<EnvIndex, E>
    where
        E: LiftFrom<InterpreterError>,
    {
        self.store.current().map_err(E::lift_from)
    }

    pub fn clone_env(&mut self, index: EnvIndex) -> Result<EnvIndex, E>
    where
        E: LiftFrom<InterpreterError>,
        V: Clone,
    {
        self.store.clone_store_from(index).map_err(E::lift_from)
    }
}

impl<'ir, S, F, C, E, Store, V> Env<V> for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    Store: Env<V>,
    E: LiftFrom<Store::Error>,
{
    type Error = E;

    fn alloc(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.store.free(index).map_err(E::lift_from)
    }

    fn read(&self, index: EnvIndex, value: kirin_ir::SSAValue) -> Result<V, Self::Error> {
        self.store.read(index, value).map_err(E::lift_from)
    }

    fn write(
        &mut self,
        index: EnvIndex,
        value: kirin_ir::SSAValue,
        data: V,
    ) -> Result<(), Self::Error> {
        self.store.write(index, value, data).map_err(E::lift_from)
    }
}

impl<'ir, S, F, C, E, Store, V> ForkEnv<V> for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    Store: ForkEnv<V>,
    E: LiftFrom<Store::Error>,
{
    fn fork_env(&mut self, index: EnvIndex) -> Result<EnvIndex, Self::Error> {
        self.store.fork_env(index).map_err(E::lift_from)
    }
}

impl<'ir, S, F, C, E, Store, L> crate::StageAccess<L>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or_else(|| E::lift_from(InterpreterError::MissingStage(stage)))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(E::lift_from)
    }
}
