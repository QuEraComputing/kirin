use kirin_ir::Dialect;

use crate::{Interpreter, Machine};

/// Dialect execution hook returning local semantic machine effects.
pub trait Interpretable<'ir, I>: Dialect
where
    I: Interpreter<'ir>,
{
    type Machine: Machine<'ir>;
    type Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, Self::Error>;
}
