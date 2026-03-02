use crate::BlockEvaluator;

/// Unified interpreter trait — automatically implemented for any type that
/// implements [`BlockEvaluator`] (which requires [`crate::ValueStore`] and
/// [`crate::StageAccess`]).
///
/// Dialect authors should use `I: Interpreter<'ir>` in their trait bounds.
/// Custom interpreter developers implement the sub-traits individually:
/// [`crate::ValueStore`], [`crate::StageAccess`], and [`BlockEvaluator`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter<'ir>: BlockEvaluator<'ir> {}

impl<'ir, T> Interpreter<'ir> for T where T: BlockEvaluator<'ir> {}
