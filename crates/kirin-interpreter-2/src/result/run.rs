use super::Suspension;

/// Outcome of a driver run loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run<S> {
    Stopped(S),
    Suspended(Suspension),
    Completed,
}
