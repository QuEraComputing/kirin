use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo};

use crate::{
    Env, EnvIndex, EnvStackStore, Frame, FrameEffect, FunctionInvocation, FunctionInvokeBuilder,
    InterpreterError, InterpreterProfile, StageFrame,
};

pub enum StepResult<C> {
    Running,
    Complete(C),
}

pub struct ConcreteInterpreter<'ir, P: InterpreterProfile> {
    pipeline: &'ir Pipeline<P::Stage>,
    frames: Vec<P::Frame>,
    envs: EnvStackStore<P::Value>,
    _marker: PhantomData<P>,
}

impl<'ir, P: InterpreterProfile> ConcreteInterpreter<'ir, P> {
    pub fn new(pipeline: &'ir Pipeline<P::Stage>) -> Self {
        Self {
            pipeline,
            frames: Vec::new(),
            envs: EnvStackStore::new(),
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

    pub fn invoke(&mut self, stage: CompileStage) -> FunctionInvokeBuilder<'_, Self> {
        FunctionInvokeBuilder::new(self, stage)
    }

    pub fn push_env(&mut self) -> EnvIndex {
        self.envs.push()
    }

    pub fn pop_env(&mut self) -> Result<EnvIndex, P::Error>
    where
        P::Error: From<InterpreterError>,
    {
        self.envs.pop().map_err(P::Error::from)
    }

    pub fn current_env(&self) -> Result<EnvIndex, P::Error>
    where
        P::Error: From<InterpreterError>,
    {
        self.envs.current().map_err(P::Error::from)
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
}

impl<'ir, P> Env<P::Value> for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    P::Value: Clone,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn alloc(&mut self) -> EnvIndex {
        self.envs.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.envs.free(index).map_err(P::Error::from)
    }

    fn read(&self, index: EnvIndex, value: kirin_ir::SSAValue) -> Result<P::Value, Self::Error> {
        self.envs.read(index, value).map_err(P::Error::from)
    }

    fn write(
        &mut self,
        index: EnvIndex,
        value: kirin_ir::SSAValue,
        data: P::Value,
    ) -> Result<(), Self::Error> {
        self.envs.write(index, value, data).map_err(P::Error::from)
    }
}

impl<'ir, P, L> crate::StageAccess<L> for ConcreteInterpreter<'ir, P>
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

#[cfg(test)]
mod tests {
    use kirin_ir::Pipeline;

    use super::*;
    use crate::{FrameEffect, HasLocation, Location};

    struct TestProfile;

    impl InterpreterProfile for TestProfile {
        type Stage = ();
        type Value = ();
        type Frame = TestFrame;
        type Completion = &'static str;
        type Error = InterpreterError;
    }

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
            ConcreteInterpreter<'ir, TestProfile>,
            TestFrame,
            &'static str,
            InterpreterError,
        > for TestFrame
    {
        fn step(
            self,
            _interp: &mut ConcreteInterpreter<'ir, TestProfile>,
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
            _interp: &mut ConcreteInterpreter<'ir, TestProfile>,
        ) -> Result<FrameEffect<TestFrame, &'static str>, InterpreterError> {
            unreachable!("test frames do not use done")
        }

        fn resume(
            self,
            completion: &'static str,
            _interp: &mut ConcreteInterpreter<'ir, TestProfile>,
        ) -> Result<FrameEffect<TestFrame, &'static str>, InterpreterError> {
            assert_eq!(self, TestFrame::Root);
            assert_eq!(completion, "child");
            Ok(FrameEffect::Complete("root"))
        }
    }

    #[test]
    fn run_resumes_parent_after_child_completion() {
        let pipeline = Pipeline::new();
        let mut interp = ConcreteInterpreter::<TestProfile>::new(&pipeline);

        interp.push_frame(TestFrame::Root);

        assert_eq!(interp.run().unwrap(), "root");
    }

    struct DoneProfile;

    impl InterpreterProfile for DoneProfile {
        type Stage = ();
        type Value = ();
        type Frame = DoneFrame;
        type Completion = &'static str;
        type Error = InterpreterError;
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
            ConcreteInterpreter<'ir, DoneProfile>,
            DoneFrame,
            &'static str,
            InterpreterError,
        > for DoneFrame
    {
        fn step(
            self,
            _interp: &mut ConcreteInterpreter<'ir, DoneProfile>,
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
            _interp: &mut ConcreteInterpreter<'ir, DoneProfile>,
        ) -> Result<FrameEffect<DoneFrame, &'static str>, InterpreterError> {
            assert_eq!(self, DoneFrame::Root);
            Ok(FrameEffect::Complete("done"))
        }

        fn resume(
            self,
            _completion: &'static str,
            _interp: &mut ConcreteInterpreter<'ir, DoneProfile>,
        ) -> Result<FrameEffect<DoneFrame, &'static str>, InterpreterError> {
            unreachable!("done test does not use completion")
        }
    }

    #[test]
    fn run_resumes_parent_after_child_done() {
        let pipeline = Pipeline::new();
        let mut interp = ConcreteInterpreter::<DoneProfile>::new(&pipeline);

        interp.push_frame(DoneFrame::Root);

        assert_eq!(interp.run().unwrap(), "done");
    }
}
