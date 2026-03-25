use kirin_ir::{CompileStage, ResultValue, SSAValue};

use crate::{Frame, InterpreterError};

/// Shared frame storage for stage-local shells.
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
        if let Some(max_depth) = self.max_depth
            && self.frames.len() >= max_depth
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
        value: SSAValue,
        result: V,
    ) -> Result<(), E> {
        self.current_mut().map(|frame| {
            frame.write_ssa(value, result);
        })
    }
}
