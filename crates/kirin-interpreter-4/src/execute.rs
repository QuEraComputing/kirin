use crate::traits::{Interpreter, Machine};

/// User-definable cursor type that encapsulates an IR traversal strategy.
///
/// Each cursor type implements `Execute<I>` with `&mut self`, tracks its own
/// traversal state, and returns effects for structural changes (push, yield,
/// return, call). Local effects (advance, jump) are handled inside `execute`.
///
/// `Execute` impls are written for concrete interpreter types (e.g.,
/// `SingleStage<...>`), not generic `I: Interpreter`. The trait genericity
/// enables sum-enum dispatch for composite cursor enums.
pub trait Execute<I: Interpreter> {
    fn execute(&mut self, interp: &mut I) -> Result<<I as Machine>::Effect, <I as Machine>::Error>;
}
