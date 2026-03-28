use kirin_ir::{ResultValue, SpecializedFunction};

use crate::{Machine, control::Shell};

use super::Interpreter;

/// Shell-side invocation of a resolved specialized function.
pub trait Invoke<'ir>: Interpreter<'ir> {
    fn invoke(
        &mut self,
        callee: SpecializedFunction,
        args: &[Self::Value],
        results: &[ResultValue],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>;

    #[allow(clippy::type_complexity)]
    fn return_current(
        &mut self,
        value: Self::Value,
    ) -> Result<
        Shell<Self::Value, <Self::Machine as Machine<'ir>>::Seed>,
        <Self as Interpreter<'ir>>::Error,
    >;
}
