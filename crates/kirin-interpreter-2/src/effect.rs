use std::marker::PhantomData;

use crate::{ConsumeEffect, ExecutionSeed, InterpreterError, Machine, control::Shell};

/// Shared shell-facing semantic flow effects for simple dialects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flow<Stop> {
    Advance,
    Stay,
    Jump(ExecutionSeed),
    Stop(Stop),
}

impl<Stop> Flow<Stop> {
    pub fn into_shell(self) -> Shell<Stop> {
        match self {
            Self::Advance => Shell::Advance,
            Self::Stay => Shell::Stay,
            Self::Jump(seed) => Shell::Replace(seed),
            Self::Stop(stop) => Shell::Stop(stop),
        }
    }

    pub fn map_stop<T>(self, f: impl FnOnce(Stop) -> T) -> Flow<T> {
        match self {
            Self::Advance => Flow::Advance,
            Self::Stay => Flow::Stay,
            Self::Jump(seed) => Flow::Jump(seed),
            Self::Stop(stop) => Flow::Stop(f(stop)),
        }
    }
}

/// Stateless machine for dialects whose effects are just shell flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stateless<Stop>(PhantomData<fn() -> Stop>);

impl<Stop> Default for Stateless<Stop> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<'ir, Stop> Machine<'ir> for Stateless<Stop> {
    type Effect = Flow<Stop>;
    type Stop = Stop;
}

impl<'ir, Stop> ConsumeEffect<'ir> for Stateless<Stop> {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        Ok(effect.into_shell())
    }
}
