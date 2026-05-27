use std::fmt::{Display, Formatter};

use kirin_arith::ArithConversionError;
use kirin_interpreter_new::{InterpreterError, LiftError};

#[derive(Debug, LiftError)]
pub enum ToyError {
    Core(InterpreterError),
    ArithConversion(ArithConversionError),
}

impl Display for ToyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => Display::fmt(error, f),
            Self::ArithConversion(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for ToyError {}

impl From<kirin_arith::interpreter_new::DivisionByZero> for ToyError {
    fn from(error: kirin_arith::interpreter_new::DivisionByZero) -> Self {
        Self::Core(error.into())
    }
}

impl From<kirin_bitwise::interpreter_new::ShiftOverflow> for ToyError {
    fn from(error: kirin_bitwise::interpreter_new::ShiftOverflow) -> Self {
        Self::Core(error.into())
    }
}
