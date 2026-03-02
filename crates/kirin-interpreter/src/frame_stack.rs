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
        if let Some(max) = self.max_depth {
            if self.frames.len() >= max {
                return Err(InterpreterError::MaxDepthExceeded.into());
            }
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
        let stage = pipeline.stage_mut(stage_id).unwrap();

        let sf = stage.staged_function().new().unwrap();
        let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
        let c0_result = c0.result;
        let ret = Return::<ArithType>::new(stage, c0_result);
        let block = stage.block().stmt(c0).terminator(ret).new();
        let region = stage.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(stage, region);
        let spec = stage.specialize().f(sf).body(body).new().unwrap();
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
}
