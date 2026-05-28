use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter::{BlockTransfer, Env, Interpretable, Location, StatementEffect};

use crate::{Cmp, CompareValue};

impl<L, I, F, C, E, T, X> Interpretable<L, I, F, C, E, X> for Cmp<T>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: CompareValue,
    <X::Value as CompareValue>::Bool: Into<X::Value>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Cmp::Eq {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .cmp_eq(&interp.read(env, *rhs)?)
                    .into();
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Cmp::Ne {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .cmp_ne(&interp.read(env, *rhs)?)
                    .into();
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Cmp::Lt {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .cmp_lt(&interp.read(env, *rhs)?)
                    .into();
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Cmp::Le {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .cmp_le(&interp.read(env, *rhs)?)
                    .into();
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Cmp::Gt {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .cmp_gt(&interp.read(env, *rhs)?)
                    .into();
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Cmp::Ge {
                lhs, rhs, result, ..
            } => {
                let value = interp
                    .read(env, *lhs)?
                    .cmp_ge(&interp.read(env, *rhs)?)
                    .into();
                interp.write(env, SSAValue::from(*result), value)?;
            }
            Cmp::__Phantom(..) => unreachable!(),
        }
        Ok(StatementEffect::Done)
    }
}
