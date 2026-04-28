use kirin::prelude::{CompileTimeValue, Dialect, PrettyPrint, SSAValue, Typeof};
use kirin_interpreter_new::{ConcreteTransfer, Env, Interpretable, Location, StatementEffect};

use crate::Constant;

impl<L, I, F, C, E, V, T, Ty> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for Constant<T, Ty>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: TryFrom<T>,
    E: From<<V as TryFrom<T>>::Error>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        let value = V::try_from(self.value.clone())?;
        interp.write(env, SSAValue::from(self.result), value)?;
        Ok(StatementEffect::Done)
    }
}
