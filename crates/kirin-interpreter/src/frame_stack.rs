use kirin_ir::{CompileStage, ResultValue, SSAValue};

use crate::{Frame, InterpreterError};

/// Shared frame storage used by interpreter runtime kernels.
#[derive(Debug)]
pub struct FrameStack<V, X> {
    frames: Vec<Frame<V, X>>,
    max_depth: Option<usize>,
}

impl<V, X> Default for FrameStack<V, X> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V, X> FrameStack<V, X> {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            max_depth: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            frames: Vec::with_capacity(capacity),
            max_depth: None,
        }
    }

    /// Set the maximum call-frame depth.
    ///
    /// Pushing beyond this limit returns [`InterpreterError::MaxDepthExceeded`].
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Frame<V, X>> {
        self.frames.iter()
    }

    pub fn current<E: From<InterpreterError>>(&self) -> Result<&Frame<V, X>, E> {
        self.frames
            .last()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub fn current_mut<E: From<InterpreterError>>(&mut self) -> Result<&mut Frame<V, X>, E> {
        self.frames
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub fn push<E: From<InterpreterError>>(&mut self, frame: Frame<V, X>) -> Result<(), E> {
        if let Some(max) = self.max_depth
            && self.frames.len() >= max
        {
            return Err(InterpreterError::MaxDepthExceeded.into());
        }
        self.frames.push(frame);
        Ok(())
    }

    pub fn pop<E: From<InterpreterError>>(&mut self) -> Result<Frame<V, X>, E> {
        self.frames
            .pop()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub fn active_stage_or(&self, fallback: CompileStage) -> CompileStage {
        self.frames.last().map(Frame::stage).unwrap_or(fallback)
    }

    pub fn read<E: From<InterpreterError>>(&self, value: SSAValue) -> Result<&V, E> {
        self.frames
            .last()
            .and_then(|frame| frame.read(value))
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    pub fn write<E: From<InterpreterError>>(
        &mut self,
        result: ResultValue,
        value: V,
    ) -> Result<(), E> {
        self.current_mut().map(|frame| {
            frame.write(result, value);
        })
    }

    pub fn write_ssa<E: From<InterpreterError>>(
        &mut self,
        ssa: SSAValue,
        value: V,
    ) -> Result<(), E> {
        self.current_mut().map(|frame| {
            frame.write_ssa(ssa, value);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;
    use kirin_arith::{ArithType, ArithValue};
    use kirin_constant::Constant;
    use kirin_function::{FunctionBody, Return};
    use kirin_ir::{Pipeline, StageInfo};
    use kirin_test_languages::CompositeLanguage;

    fn build_fixture() -> (
        Pipeline<StageInfo<CompositeLanguage>>,
        kirin_ir::CompileStage,
        kirin_ir::SpecializedFunction,
        kirin_ir::Block,
        kirin_ir::ResultValue,
    ) {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
        let (spec, block, c0_result) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
            let sf = b.staged_function().new().unwrap();
            let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
            let c0_result = c0.result;
            let ret = Return::<ArithType>::new(b, c0_result);
            let block = b.block().stmt(c0).terminator(ret).new();
            let region = b.region().add_block(block).new();
            let body = FunctionBody::<ArithType>::new(
                b,
                region,
                kirin_ir::Signature::new(vec![], ArithType::default(), ()),
            );
            let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
            (spec, block, c0_result)
        });
        (pipeline, stage_id, spec, block, c0_result)
    }

    #[test]
    fn test_frame_stack_invariants() {
        let (_pipeline, stage_id, callee, _block, result) = build_fixture();
        let mut stack: FrameStack<i32, ()> = FrameStack::new();

        assert_eq!(stack.depth(), 0);
        assert_eq!(stack.active_stage_or(stage_id), stage_id);

        stack
            .push::<InterpreterError>(Frame::new(callee, stage_id, ()))
            .unwrap();
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.active_stage_or(stage_id), stage_id);

        stack.write::<InterpreterError>(result, 7).unwrap();
        let got: &i32 = stack.read::<InterpreterError>(result.into()).unwrap();
        assert_eq!(*got, 7);

        let popped: Frame<i32, ()> = stack.pop::<InterpreterError>().unwrap();
        assert_eq!(popped.callee(), callee);
        assert_eq!(stack.depth(), 0);
    }

    #[test]
    fn test_frame_stack_max_depth_exceeded() {
        let (_pipeline, stage_id, callee, _block, _result) = build_fixture();
        let mut stack: FrameStack<i32, ()> = FrameStack::new().with_max_depth(1);

        stack
            .push::<InterpreterError>(Frame::new(callee, stage_id, ()))
            .unwrap();
        assert_eq!(stack.depth(), 1);

        let err = stack
            .push::<InterpreterError>(Frame::new(callee, stage_id, ()))
            .unwrap_err();
        assert!(
            matches!(err, InterpreterError::MaxDepthExceeded),
            "expected MaxDepthExceeded, got {err:?}"
        );
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_frame_stack_pop_empty_returns_no_frame() {
        let mut stack: FrameStack<i32, ()> = FrameStack::new();
        let err = stack.pop::<InterpreterError>().unwrap_err();
        assert!(
            matches!(err, InterpreterError::NoFrame),
            "expected NoFrame, got {err:?}"
        );
    }

    #[test]
    fn test_frame_stack_current_empty_returns_no_frame() {
        let stack: FrameStack<i32, ()> = FrameStack::new();
        let err = stack.current::<InterpreterError>().unwrap_err();
        assert!(
            matches!(err, InterpreterError::NoFrame),
            "expected NoFrame, got {err:?}"
        );
    }

    #[test]
    fn test_frame_stack_read_unbound_value() {
        use kirin_ir::TestSSAValue;

        let (_pipeline, stage_id, callee, _block, _result) = build_fixture();
        let mut stack: FrameStack<i32, ()> = FrameStack::new();
        stack
            .push::<InterpreterError>(Frame::new(callee, stage_id, ()))
            .unwrap();

        let bogus = SSAValue::from(TestSSAValue(9999));
        let err = stack.read::<InterpreterError>(bogus).unwrap_err();
        assert!(
            matches!(err, InterpreterError::UnboundValue(v) if v == bogus),
            "expected UnboundValue, got {err:?}"
        );
    }

    #[test]
    fn test_frame_stack_write_ssa_and_read_back() {
        use kirin_ir::TestSSAValue;

        let (_pipeline, stage_id, callee, _block, _result) = build_fixture();
        let mut stack: FrameStack<i32, ()> = FrameStack::new();
        stack
            .push::<InterpreterError>(Frame::new(callee, stage_id, ()))
            .unwrap();

        let ssa = SSAValue::from(TestSSAValue(42));
        stack.write_ssa::<InterpreterError>(ssa, 123).unwrap();
        let got = stack.read::<InterpreterError>(ssa).unwrap();
        assert_eq!(*got, 123);
    }

    #[test]
    fn test_frame_stack_active_stage_follows_top() {
        let (_pipeline1, stage1, callee1, _block1, _result1) = build_fixture();
        // Build a second fixture to get a different stage
        let (_pipeline2, stage2, callee2, _block2, _result2) = build_fixture();

        let mut stack: FrameStack<i32, ()> = FrameStack::new();
        let fallback = stage1;

        // Empty stack returns fallback
        assert_eq!(stack.active_stage_or(fallback), fallback);

        // Push first frame
        stack
            .push::<InterpreterError>(Frame::new(callee1, stage1, ()))
            .unwrap();
        assert_eq!(stack.active_stage_or(fallback), stage1);

        // Push second frame with different stage
        stack
            .push::<InterpreterError>(Frame::new(callee2, stage2, ()))
            .unwrap();
        assert_eq!(stack.active_stage_or(fallback), stage2);

        // Pop second, should revert to first
        stack.pop::<InterpreterError>().unwrap();
        assert_eq!(stack.active_stage_or(fallback), stage1);
    }

    #[test]
    fn test_frame_stack_with_capacity() {
        let stack: FrameStack<i32, ()> = FrameStack::with_capacity(16);
        assert!(stack.is_empty());
        assert_eq!(stack.depth(), 0);
    }
}
