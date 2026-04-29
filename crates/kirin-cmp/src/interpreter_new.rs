use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter_new::{BlockTransfer, Env, Interpretable, Location, StatementEffect};

use crate::{Cmp, CompareValue};

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for Cmp<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: CompareValue,
    <V as CompareValue>::Bool: Into<V>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
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
