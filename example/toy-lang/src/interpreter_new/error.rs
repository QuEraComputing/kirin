use std::convert::Infallible;
use std::fmt::{Display, Formatter};

use kirin::prelude::TryLiftFrom;
use kirin_arith::ArithConversionError;
use kirin_interpreter_new::InterpreterError;

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

impl TryLiftFrom<InterpreterError> for ToyError {
    type Error = Infallible;

    fn try_lift_from(error: InterpreterError) -> Result<Self, Self::Error> {
        Ok(Self::Core(error))
    }
}

impl From<ArithConversionError> for ToyError {
    fn from(error: ArithConversionError) -> Self {
        Self::ArithConversion(error)
    }
}

impl TryLiftFrom<ArithConversionError> for ToyError {
    type Error = Infallible;

    fn try_lift_from(error: ArithConversionError) -> Result<Self, Self::Error> {
        Ok(Self::ArithConversion(error))
    }
}

impl From<Infallible> for ToyError {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl TryLiftFrom<Infallible> for ToyError {
    type Error = Infallible;

    fn try_lift_from(error: Infallible) -> Result<Self, Self::Error> {
        match error {}
    }
}

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
