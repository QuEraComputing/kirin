use kirin_interpreter::{AbstractEnvStore, InterpreterProfile};

use crate::ConstPropFixpointInterpreter;

/// Standard const-prop function fixpoint alias.
///
/// Wraps [`ConstPropFixpointInterpreter`] with the default
/// [`AbstractEnvStore`] for the SSA store. The supplied profile is expected
/// to bind `SummaryKey = ConstPropOwner` and `Summary = ConstPropSummary<...>`.
pub type ConstPropFunctionFixpoint<'ir, P> =
    ConstPropFixpointInterpreter<'ir, P, AbstractEnvStore<<P as InterpreterProfile>::Value>>;
