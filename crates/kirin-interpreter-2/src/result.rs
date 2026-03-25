use crate::Control;

/// Result of one executed statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult<E, S> {
    effect: E,
    control: Control<S>,
}

impl<E, S> StepResult<E, S> {
    pub fn new(effect: E, control: Control<S>) -> Self {
        Self { effect, control }
    }

    pub fn effect(&self) -> &E {
        &self.effect
    }

    pub fn control(&self) -> &Control<S> {
        &self.control
    }

    pub fn into_parts(self) -> (E, Control<S>) {
        (self.effect, self.control)
    }
}

/// Outcome of one driver-level step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome<E, S> {
    Stepped(StepResult<E, S>),
    Suspended(SuspendReason),
    Completed,
}

/// Outcome of a driver run loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunResult<S> {
    Stopped(S),
    Suspended(SuspendReason),
    Completed,
}

/// Framework-owned suspension reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuspendReason {
    Breakpoint,
    FuelExhausted,
    HostInterrupt,
}
