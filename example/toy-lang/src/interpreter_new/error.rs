use std::convert::Infallible;
use std::fmt::{Display, Formatter};

use kirin::prelude::TryLiftFrom;
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

impl TryLiftFrom<kirin_arith::interpreter_new::DivisionByZero> for ToyError {
    type Error = Infallible;

    fn try_lift_from(
        error: kirin_arith::interpreter_new::DivisionByZero,
    ) -> Result<Self, Self::Error> {
        Ok(Self::Core(InterpreterError::try_lift_from(error)?))
    }
}

impl TryLiftFrom<kirin_bitwise::interpreter_new::ShiftOverflow> for ToyError {
    type Error = Infallible;

    fn try_lift_from(
        error: kirin_bitwise::interpreter_new::ShiftOverflow,
    ) -> Result<Self, Self::Error> {
        Ok(Self::Core(InterpreterError::try_lift_from(error)?))
    }
}
