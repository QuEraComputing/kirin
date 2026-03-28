use crate::control::Shell;

use super::Suspension;

/// Result payload for one executed statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stepped<E, S, Seed = ()> {
    effect: E,
    control: Shell<S, Seed>,
}

impl<E, S, Seed> Stepped<E, S, Seed> {
    pub fn new(effect: E, control: Shell<S, Seed>) -> Self {
        Self { effect, control }
    }

    pub fn effect(&self) -> &E {
        &self.effect
    }

    pub fn control(&self) -> &Shell<S, Seed> {
        &self.control
    }

    pub fn into_parts(self) -> (E, Shell<S, Seed>) {
        (self.effect, self.control)
    }
}

/// Outcome of one driver-level step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step<E, S, Seed = ()> {
    Stepped(Stepped<E, S, Seed>),
    Suspended(Suspension),
    Completed,
}
