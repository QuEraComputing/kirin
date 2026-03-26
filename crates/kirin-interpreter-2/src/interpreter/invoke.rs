use kirin_ir::{ResultValue, SpecializedFunction};

use crate::effect;

use super::Interpreter;

/// Shell-side invocation of a resolved specialized function.
pub trait Invoke<'ir>: Interpreter<'ir> {
    fn invoke(
        &mut self,
        callee: SpecializedFunction,
        args: &[Self::Value],
        results: &[ResultValue],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>;

    fn return_current(
        &mut self,
        value: Self::Value,
    ) -> Result<effect::Flow<Self::Value>, <Self as Interpreter<'ir>>::Error>;
}
