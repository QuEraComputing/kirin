use kirin_ir::{ResultValue, SSAValue};

use crate::InterpretControl;

/// Error constructors for interpreter failures.
///
/// Implementors define how to produce errors for common interpreter
/// failure modes. Used by [`crate::StackInterpreter`] (and future
/// interpreter types) to convert internal failures into the user's
/// error type without storing function pointers.
pub trait InterpreterError {
    /// No call frame on the stack.
    fn no_frame() -> Self;

    /// An SSA value was read before being written.
    fn unbound_value(value: SSAValue) -> Self;

    /// Step fuel has been exhausted.
    fn fuel_exhausted() -> Self;

    /// Call depth exceeded the configured maximum.
    fn max_depth_exceeded() -> Self;

    /// Function entry block could not be resolved.
    fn missing_entry() -> Self;

    /// An unexpected control flow action was encountered.
    fn unexpected_control(msg: &str) -> Self;
}

/// Minimal state contract for interpreter implementations.
///
/// Provides SSA value read/write only. The associated `Control` type
/// determines which control flow actions are available — concrete
/// interpreters use [`crate::ConcreteControl`] while abstract interpreters
/// use [`crate::AbstractControl`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter: Sized {
    type Value;
    type Error;
    type Control: InterpretControl<Self::Value>;

    /// Returns a reference to the bound value without cloning.
    fn read_ref(&self, value: SSAValue) -> Result<&Self::Value, Self::Error>;

    /// Returns a cloned copy of the bound value.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>
    where
        Self::Value: Clone,
    {
        self.read_ref(value).cloned()
    }

    /// Bind a result to a value.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
}
