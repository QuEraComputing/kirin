use kirin_ir::{ResultValue, SpecializedFunction};

use crate::{Machine, control::Directive};

use super::Interpreter;

/// Interpreter-side invocation of a resolved specialized function.
pub trait Invoke<'ir>: Interpreter<'ir> {
    fn invoke(
        &mut self,
        callee: SpecializedFunction,
        args: &[Self::Value],
        results: &[ResultValue],
    ) -> Result<(), <Self as crate::ValueStore>::Error>;

    #[allow(clippy::type_complexity)]
    fn return_current(
        &mut self,
        value: Self::Value,
    ) -> Result<
        Directive<Self::Value, <Self as Machine<'ir>>::Seed>,
        <Self as crate::ValueStore>::Error,
    >;
}
