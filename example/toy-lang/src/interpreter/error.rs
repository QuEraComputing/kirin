use std::convert::Infallible;
use std::fmt::{Display, Formatter};

use kirin_arith::ArithConversionError;
use kirin_interpreter::InterpreterError;

#[derive(Debug)]
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

impl From<InterpreterError> for ToyError {
    fn from(error: InterpreterError) -> Self {
        Self::Core(error)
    }
}

impl From<ArithConversionError> for ToyError {
    fn from(error: ArithConversionError) -> Self {
        Self::ArithConversion(error)
    }
}

impl From<Infallible> for ToyError {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<kirin_arith::interpreter::DivisionByZero> for ToyError {
    fn from(error: kirin_arith::interpreter::DivisionByZero) -> Self {
        Self::Core(error.into())
    }
}

impl From<kirin_bitwise::interpreter::ShiftOverflow> for ToyError {
    fn from(error: kirin_bitwise::interpreter::ShiftOverflow) -> Self {
        Self::Core(error.into())
    }
}
