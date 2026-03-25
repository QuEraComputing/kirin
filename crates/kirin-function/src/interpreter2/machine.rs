use kirin::prelude::{ResultValue, SSAValue};
use kirin_interpreter_2::{
    ConsumeEffect, ExecutionSeed, InterpreterError, Machine as RuntimeMachine, control::Shell,
};

use super::Effect;

/// One suspended caller frame in the function machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame<V> {
    bindings: Vec<(SSAValue, V)>,
    results: Vec<ResultValue>,
    resume: ExecutionSeed,
}

impl<V> CallFrame<V> {
    pub fn new(
        bindings: Vec<(SSAValue, V)>,
        results: Vec<ResultValue>,
        resume: ExecutionSeed,
    ) -> Self {
        Self {
            bindings,
            results,
            resume,
        }
    }

    pub fn into_parts(self) -> (Vec<(SSAValue, V)>, Vec<ResultValue>, ExecutionSeed) {
        (self.bindings, self.results, self.resume)
    }
}

/// Same-stage function call stack for `kirin-interpreter-2`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine<V> {
    frames: Vec<CallFrame<V>>,
}

impl<V> Machine<V> {
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    pub fn frame_depth(&self) -> usize {
        self.frames.len()
    }

    pub fn push_frame(&mut self, frame: CallFrame<V>) {
        self.frames.push(frame);
    }

    pub fn pop_frame(&mut self) -> Option<CallFrame<V>> {
        self.frames.pop()
    }
}

impl<'ir, V> RuntimeMachine<'ir> for Machine<V> {
    type Effect = Effect<V>;
    type Stop = V;
}

impl<'ir, V> ConsumeEffect<'ir> for Machine<V> {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        Ok(effect.into_shell())
    }
}
