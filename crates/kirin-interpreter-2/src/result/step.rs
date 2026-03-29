use super::Suspension;

/// Outcome of one driver-level step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step<E> {
    Stepped(E),
    Suspended(Suspension),
    Completed,
}
