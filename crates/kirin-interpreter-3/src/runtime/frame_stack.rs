use crate::InterpreterError;

use super::frame::Frame;

#[derive(Debug, Default, Clone)]
pub(crate) struct FrameStack<V> {
    frames: Vec<Frame<V>>,
}

impl<V> FrameStack<V> {
    pub(crate) fn new() -> Self {
        Self { frames: Vec::new() }
    }

    pub(crate) fn clear(&mut self) {
        self.frames.clear();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub(crate) fn push(&mut self, frame: Frame<V>) {
        self.frames.push(frame);
    }

    pub(crate) fn pop(&mut self) -> Result<Frame<V>, InterpreterError> {
        self.frames.pop().ok_or(InterpreterError::NoFrame)
    }

    pub(crate) fn current(&self) -> Result<&Frame<V>, InterpreterError> {
        self.frames.last().ok_or(InterpreterError::NoFrame)
    }

    pub(crate) fn current_mut(&mut self) -> Result<&mut Frame<V>, InterpreterError> {
        self.frames.last_mut().ok_or(InterpreterError::NoFrame)
    }
}
