use crate::control::Shell;

use super::Suspension;

/// Result payload for one executed statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stepped<E, S> {
    effect: E,
    control: Shell<S>,
}

impl<E, S> Stepped<E, S> {
    pub fn new(effect: E, control: Shell<S>) -> Self {
        Self { effect, control }
    }

    pub fn effect(&self) -> &E {
        &self.effect
    }

    pub fn control(&self) -> &Shell<S> {
        &self.control
    }

    pub fn into_parts(self) -> (E, Shell<S>) {
        (self.effect, self.control)
    }
}

/// Outcome of one driver-level step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step<E, S> {
    Stepped(Stepped<E, S>),
    Suspended(Suspension),
    Completed,
}
