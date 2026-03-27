use std::{convert::Infallible, marker::PhantomData};

use crate::{
    ConsumeEffect, ExecutionSeed, InterpreterError, Lift, LiftEffect, LiftStop, Machine,
    control::Shell,
};

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

impl Flow<Infallible> {
    /// Upcast a total (never-stopping) flow into any target stop type.
    pub fn upcast<T>(self) -> Flow<T> {
        match self {
            Self::Advance => Flow::Advance,
            Self::Stay => Flow::Stay,
            Self::Jump(seed) => Flow::Jump(seed),
            // Infallible has no variants, so this arm is unreachable.
            Self::Stop(never) => match never {},
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

/// Any `Stateless<S>` can lift from a total (never-stopping) `Stateless<Infallible>`.
impl<'ir, S> LiftEffect<'ir, Stateless<Infallible>> for Stateless<S> {
    fn lift_effect(effect: Flow<Infallible>) -> Flow<S> {
        effect.upcast()
    }
}

impl<'ir, S> LiftStop<'ir, Stateless<Infallible>> for Stateless<S> {
    fn lift_stop(stop: Infallible) -> S {
        match stop {}
    }
}

/// Cursor directive for total (non-stopping) dialect operations.
///
/// Contains only cursor directives — no Stop variant. Total dialects
/// return `Cursor` instead of `Flow<Infallible>`, which avoids Lift
/// trait overlap between identity and Infallible-upcast impls.
///
/// `Cursor` intentionally omits a `Push` variant — total dialects do not
/// spawn nested execution contexts. If needed in the future, extend here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// Advance to the next statement in the block.
    Advance,
    /// Stay at the current cursor position (operation already moved it).
    Stay,
    /// Jump to a different execution point.
    Jump(ExecutionSeed),
}

/// Lift a total cursor directive into any `Flow<S>`.
impl<S> Lift<Flow<S>> for Cursor {
    fn lift(self) -> Flow<S> {
        match self {
            Self::Advance => Flow::Advance,
            Self::Stay => Flow::Stay,
            Self::Jump(seed) => Flow::Jump(seed),
        }
    }
}
