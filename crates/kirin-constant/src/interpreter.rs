use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter::dialect::{ForwardEffect, ForwardInterp, Interpretable, ValueContext};

use crate::Constant;

impl<I, T, Ty> Interpretable<ValueContext<'_, I>> for Constant<T, Ty>
where
    I: ForwardInterp,
    I::Value: TryFrom<T>,
    I::Error: From<<I::Value as TryFrom<T>>::Error>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        let value = I::Value::try_from(self.value.clone()).map_err(I::Error::from)?;
        ctx.write(self.result, value)?;
        Ok(ForwardEffect::Next)
    }
}
