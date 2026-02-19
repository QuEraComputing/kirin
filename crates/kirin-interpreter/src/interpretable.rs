use crate::{Continuation, Interpreter};
use kirin_ir::Dialect;

/// Dialect execution hook. Each dialect implements this to define how its
/// operations affect interpreter state and which continuation happens next.
///
/// Dialect impls construct [`Continuation`] variants directly:
/// `Continuation::Continue`, `Continuation::Jump(block, args)`, etc.
pub trait Interpretable<I>: Dialect
where
    I: Interpreter,
{
    fn interpret(&self, interpreter: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>;
}
