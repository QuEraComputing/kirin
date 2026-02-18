use crate::Interpreter;
use kirin_ir::Dialect;

/// Dialect execution hook. Each dialect implements this to define how its
/// operations affect interpreter state and which control action happens next.
///
/// The return type `I::Control` is determined by the interpreter:
/// - [`crate::StackInterpreter`] uses [`crate::ConcreteControl`]
/// - [`crate::AbstractInterpreter`] uses [`crate::AbstractControl`]
///
/// Dialect impls that only need shared control actions (`Continue`, `Jump`,
/// `Return`, `Call`) can be generic over `I: Interpreter` and use
/// [`crate::InterpretControl`] constructors like `I::Control::ctrl_continue()`.
pub trait Interpretable<I>: Dialect
where
    I: Interpreter,
{
    fn interpret(&self, interpreter: &mut I) -> Result<I::Control, I::Error>;
}
