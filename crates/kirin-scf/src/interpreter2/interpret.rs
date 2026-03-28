use kirin::prelude::CompileTimeValue;
use kirin_interpreter_2::{Cursor, Interpretable, Interpreter, InterpreterError};

use crate::Yield;

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for Yield<T>
where
    I: Interpreter<'ir>,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Cursor, Self::Error> {
        Err(unsupported(
            "scf.yield has no independent semantics; \
             it may only appear as a terminator inside scf.if or scf.for body blocks",
        )
        .into())
    }
}
