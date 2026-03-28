use crate::control::Directive;

use super::Suspension;

/// Result payload for one executed statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stepped<E, S, Seed = ()> {
    effect: E,
    control: Directive<S, Seed>,
}

impl<E, S, Seed> Stepped<E, S, Seed> {
    pub fn new(effect: E, control: Directive<S, Seed>) -> Self {
        Self { effect, control }
    }

    pub fn effect(&self) -> &E {
        &self.effect
    }

    pub fn control(&self) -> &Directive<S, Seed> {
        &self.control
    }

    pub fn into_parts(self) -> (E, Directive<S, Seed>) {
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
