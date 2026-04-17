use crate::error::InterpreterError;
use crate::frame::Frame;
use kirin_ir::{CompileStage, ResultValue, SSAValue};

pub struct FrameStack<V> {
    frames: Vec<Frame<V>>,
    max_depth: Option<usize>,
}

impl<V> Default for FrameStack<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> FrameStack<V> {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            max_depth: None,
        }
    }

    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = Some(max_depth);
        self
    }

    pub fn push(&mut self, frame: Frame<V>) -> Result<(), InterpreterError> {
        if let Some(max) = self.max_depth
            && self.frames.len() >= max
        {
            return Err(InterpreterError::MaxDepthExceeded);
        }
        self.frames.push(frame);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<Frame<V>> {
        self.frames.pop()
    }

    pub fn current(&self) -> Option<&Frame<V>> {
        self.frames.last()
    }

    pub fn current_mut(&mut self) -> Option<&mut Frame<V>> {
        self.frames.last_mut()
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

// ValueStore-like helpers that delegate to the top frame
impl<V: Clone> FrameStack<V> {
    pub fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        let frame = self.current().ok_or(InterpreterError::NoFrame)?;
        frame
            .read(ssa)
            .cloned()
            .ok_or(InterpreterError::UnboundValue(ssa))
    }

    pub fn write(&mut self, result: ResultValue, value: V) -> Result<(), InterpreterError> {
        let frame = self.current_mut().ok_or(InterpreterError::NoFrame)?;
        frame.write(result, value);
        Ok(())
    }

    pub fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), InterpreterError> {
        let frame = self.current_mut().ok_or(InterpreterError::NoFrame)?;
        frame.write_ssa(ssa, value);
        Ok(())
    }

    /// Get the active stage from the top frame, falling back to a root stage.
    pub fn active_stage_or(&self, root: CompileStage) -> CompileStage {
        self.current().map_or(root, |f| f.stage())
    }
}
