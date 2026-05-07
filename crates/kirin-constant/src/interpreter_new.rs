use kirin::prelude::{CompileTimeValue, Dialect, LiftFrom, PrettyPrint, SSAValue, Typeof};
use kirin_interpreter_new::{BlockTransfer, Env, Interpretable, Location, StatementEffect};

use crate::Constant;

impl<L, I, F, C, E, T, Ty, X> Interpretable<L, I, F, C, E, X> for Constant<T, Ty>
where
    L: Dialect,
    I: Env<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: TryFrom<T>,
    E: LiftFrom<<X::Value as TryFrom<T>>::Error>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        let value = X::Value::try_from(self.value.clone()).map_err(E::lift_from)?;
        interp.write(env, SSAValue::from(self.result), value)?;
        Ok(StatementEffect::Done)
    }
}
