use kirin_ir::SpecializedFunction;

use super::Interpreter;

/// Request-side call resolution for call-like statements.
pub trait ResolveCall<'ir, I: Interpreter<'ir>> {
    fn resolve_call(
        &self,
        interp: &I,
        args: &[I::Value],
    ) -> Result<SpecializedFunction, <I as Interpreter<'ir>>::Error>;
}
