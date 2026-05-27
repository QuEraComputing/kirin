use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo};

use crate::{
    Env, EnvIndex, EnvStackStore, Frame, FrameEffect, FunctionInvocation, FunctionInvocationFrame,
    FunctionInvokeBuilder, InterpreterError,
};

pub enum StepResult<C> {
    Running,
    Complete(C),
}

pub struct ConcreteInterpreter<'ir, S, F, C, E, V> {
    pipeline: &'ir Pipeline<S>,
    frames: Vec<F>,
    envs: EnvStackStore<V>,
    _marker: PhantomData<(C, E)>,
}

impl<'ir, S, F, C, E, V> ConcreteInterpreter<'ir, S, F, C, E, V> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            frames: Vec::new(),
            envs: EnvStackStore::new(),
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

    pub fn invoke(&mut self, stage: CompileStage) -> FunctionInvokeBuilder<'_, Self> {
        FunctionInvokeBuilder::new(self, stage)
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

    pub fn run_function_invocation(&mut self, invocation: FunctionInvocation<V>) -> Result<C, E>
    where
        F: FunctionInvocationFrame<V> + Frame<Self, F, C, E>,
        E: From<InterpreterError> + From<<F as FunctionInvocationFrame<V>>::Error>,
    {
        self.push_frame(F::from_function_invocation(invocation).map_err(E::from)?);
        self.run()
    }

    /// Resolve a function by name in the given stage and run it with the
    /// given arguments. Returns the raw completion produced by the function.
    pub fn run_function_by_name<A>(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: A,
    ) -> Result<C, E>
    where
        S: kirin_ir::StageMeta,
        Self: crate::FunctionInvocationDispatch<F, E, V>,
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
        A: IntoIterator<Item = V>,
    {
        let stage = self
            .pipeline
            .stage_by_name(stage_name)
            .ok_or_else(|| InterpreterError::MissingStageName(stage_name.into()))
            .map_err(E::from)?;
        let function = self
            .pipeline
            .lookup_function_by_name(function_name)
            .ok_or_else(|| InterpreterError::MissingFunctionName(function_name.into()))
            .map_err(E::from)?;
        self.invoke(stage).function(function).args(args)
    }

    pub fn step(&mut self) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        let frame = match self.frames.pop() {
            Some(frame) => frame,
            None => return Err(E::from(InterpreterError::EmptyFrameStack)),
        };
        let effect = frame.step(self)?;
        self.apply_effect(effect)
    }

    fn apply_effect(&mut self, mut effect: FrameEffect<F, C>) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
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
                        None => return Err(E::from(InterpreterError::EmptyFrameStack)),
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
}

impl<'ir, S, F, C, E, V> Env<V> for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    V: Clone,
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

impl<'ir, S, F, C, E, V, L> crate::StageAccess<L> for ConcreteInterpreter<'ir, S, F, C, E, V>
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
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(E::from)
    }
}

#[cfg(test)]
mod tests {
    use kirin_ir::Pipeline;

    use super::*;
    use crate::{FrameEffect, HasLocation, Location};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestFrame {
        Root,
        Child,
    }

    impl HasLocation for TestFrame {
        fn location(&self) -> Location {
            panic!("test frame has no IR location")
        }
    }

    impl<'ir>
        crate::Frame<
            ConcreteInterpreter<'ir, (), TestFrame, &'static str, InterpreterError, ()>,
            TestFrame,
            &'static str,
            InterpreterError,
        > for TestFrame
    {
        fn step(
            self,
            _interp: &mut ConcreteInterpreter<
                'ir,
                (),
                TestFrame,
                &'static str,
                InterpreterError,
                (),
            >,
        ) -> Result<FrameEffect<TestFrame, &'static str>, InterpreterError> {
            match self {
                TestFrame::Root => Ok(FrameEffect::Push {
                    parent: TestFrame::Root,
                    child: TestFrame::Child,
                }),
                TestFrame::Child => Ok(FrameEffect::Complete("child")),
            }
        }

        fn resume_done(
            self,
            _interp: &mut ConcreteInterpreter<
                'ir,
                (),
                TestFrame,
                &'static str,
                InterpreterError,
                (),
            >,
        ) -> Result<FrameEffect<TestFrame, &'static str>, InterpreterError> {
            unreachable!("test frames do not use done")
        }

        fn resume(
            self,
            completion: &'static str,
            _interp: &mut ConcreteInterpreter<
                'ir,
                (),
                TestFrame,
                &'static str,
                InterpreterError,
                (),
            >,
        ) -> Result<FrameEffect<TestFrame, &'static str>, InterpreterError> {
            assert_eq!(self, TestFrame::Root);
            assert_eq!(completion, "child");
            Ok(FrameEffect::Complete("root"))
        }
    }

    #[test]
    fn run_resumes_parent_after_child_completion() {
        let pipeline = Pipeline::new();
        let mut interp = ConcreteInterpreter::new(&pipeline);

        interp.push_frame(TestFrame::Root);

        assert_eq!(interp.run().unwrap(), "root");
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum DoneFrame {
        Root,
        Child,
    }

    impl HasLocation for DoneFrame {
        fn location(&self) -> Location {
            panic!("test frame has no IR location")
        }
    }

    impl<'ir>
        crate::Frame<
            ConcreteInterpreter<'ir, (), DoneFrame, &'static str, InterpreterError, ()>,
            DoneFrame,
            &'static str,
            InterpreterError,
        > for DoneFrame
    {
        fn step(
            self,
            _interp: &mut ConcreteInterpreter<
                'ir,
                (),
                DoneFrame,
                &'static str,
                InterpreterError,
                (),
            >,
        ) -> Result<FrameEffect<DoneFrame, &'static str>, InterpreterError> {
            match self {
                DoneFrame::Root => Ok(FrameEffect::Push {
                    parent: DoneFrame::Root,
                    child: DoneFrame::Child,
                }),
                DoneFrame::Child => Ok(FrameEffect::Done),
            }
        }

        fn resume_done(
            self,
            _interp: &mut ConcreteInterpreter<
                'ir,
                (),
                DoneFrame,
                &'static str,
                InterpreterError,
                (),
            >,
        ) -> Result<FrameEffect<DoneFrame, &'static str>, InterpreterError> {
            assert_eq!(self, DoneFrame::Root);
            Ok(FrameEffect::Complete("done"))
        }

        fn resume(
            self,
            _completion: &'static str,
            _interp: &mut ConcreteInterpreter<
                'ir,
                (),
                DoneFrame,
                &'static str,
                InterpreterError,
                (),
            >,
        ) -> Result<FrameEffect<DoneFrame, &'static str>, InterpreterError> {
            unreachable!("done test does not use completion")
        }
    }

    #[test]
    fn run_resumes_parent_after_child_done() {
        let pipeline = Pipeline::new();
        let mut interp = ConcreteInterpreter::new(&pipeline);

        interp.push_frame(DoneFrame::Root);

        assert_eq!(interp.run().unwrap(), "done");
    }
}
