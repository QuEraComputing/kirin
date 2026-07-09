use kirin::prelude::{CompileTimeValue, PrettyPrint, Typeof};
use kirin_interpreter::dialect::{ForwardEffect, ForwardEval, ForwardEvalInterp, Interpretable};

use crate::Constant;

impl<I, T, Ty> Interpretable<I, ForwardEval> for Constant<T, Ty>
where
    I: ForwardEvalInterp,
    I::Value: TryFrom<T>,
    I::Error: From<<I::Value as TryFrom<T>>::Error>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let value = I::Value::try_from(self.value.clone()).map_err(I::Error::from)?;
        interp.write(self.result, value)?;
        Ok(ForwardEffect::Next)
    }
}
