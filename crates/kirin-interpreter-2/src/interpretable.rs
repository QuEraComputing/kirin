use kirin_ir::Dialect;

use crate::Interpreter;

/// Dialect execution hook returning local semantic effects.
pub trait Interpretable<'ir, I>: Dialect
where
    I: Interpreter<'ir>,
{
    type Effect;
    type Error;

    fn interpret(&self, interp: &mut I) -> Result<Self::Effect, Self::Error>;
}
