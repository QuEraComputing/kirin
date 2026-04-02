use kirin::prelude::CompileTimeValue;
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{Interpretable, Interpreter, Machine, ValueStore};

use crate::{Cmp, CompareValue};

impl<I, T> Interpretable<I> for Cmp<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: CompareValue,
    <<I as ValueStore>::Value as CompareValue>::Bool: Into<<I as ValueStore>::Value>,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = CursorEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<CursorEffect<<I as ValueStore>::Value>, InterpreterError> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs.cmp_eq(&rhs).into())?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs.cmp_ne(&rhs).into())?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs.cmp_lt(&rhs).into())?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs.cmp_le(&rhs).into())?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs.cmp_gt(&rhs).into())?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let lhs = interp.read(*lhs)?;
                let rhs = interp.read(*rhs)?;
                interp.write(*result, lhs.cmp_ge(&rhs).into())?;
            }
            Self::__Phantom(..) => unreachable!(),
        }

        Ok(CursorEffect::Advance)
    }
}
