use kirin::prelude::{CompileTimeValue, HasRegionBody, Product, SSAValue};
use kirin_interpreter::dialect::{
    CallEffect, Callee, ForwardEffect, ForwardInterp, FunctionBody, FunctionEntry, Interp,
    Interpretable, InterpreterError, ValueContext,
};

use crate::{
    Bind, CallFunction, CallLike, CallNamed, CallSpecialized, CallStaged, Function, Lambda, Return,
};

impl<I, T> FunctionEntry<ValueContext<'_, I>> for Function<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn function_entry(
        &self,
        args: Product<I::Value>,
        _ctx: &mut ValueContext<'_, I>,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        Ok(FunctionBody::new(*self.region()).args(args))
    }
}

impl<I, T> FunctionEntry<ValueContext<'_, I>> for Lambda<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn function_entry(
        &self,
        args: Product<I::Value>,
        _ctx: &mut ValueContext<'_, I>,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        Ok(FunctionBody::new(*self.region()).args(args))
    }
}

/// Function definitions are inert at runtime: defining a function does not
/// execute its body. Bodies run when the function is invoked (via
/// [`FunctionEntry`]).
impl<I, T> Interpretable<ValueContext<'_, I>> for Function<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        Ok(ForwardEffect::Next)
    }
}

impl<I, T> Interpretable<ValueContext<'_, I>> for Lambda<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "first-class lambda values are not yet supported",
        )))
    }
}

impl<I, T> Interpretable<ValueContext<'_, I>> for Bind<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "bind is not yet supported by the interpreter",
        )))
    }
}

fn call_effect<I, T, C>(
    call: &C,
    callee: Callee,
    ctx: &mut ValueContext<'_, I>,
) -> Result<I::Effect, I::Error>
where
    I: ForwardInterp,
    T: CompileTimeValue,
    C: CallLike<T>,
{
    let args = call
        .arguments()
        .map(|argument| ctx.read(*argument))
        .collect::<Result<Product<_>, _>>()?;
    let results = call.results().copied().map(SSAValue::from).collect();
    Ok(ForwardEffect::Call(CallEffect {
        callee,
        stage: call.stage(),
        args,
        results,
    }))
}

impl<I, T> Interpretable<ValueContext<'_, I>> for CallNamed<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Named(self.target()), ctx)
    }
}

impl<I, T> Interpretable<ValueContext<'_, I>> for CallFunction<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Function(self.target()), ctx)
    }
}

impl<I, T> Interpretable<ValueContext<'_, I>> for CallStaged<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Staged(self.target()), ctx)
    }
}

impl<I, T> Interpretable<ValueContext<'_, I>> for CallSpecialized<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Specialized(self.target()), ctx)
    }
}

impl<I, T> Interpretable<ValueContext<'_, I>> for Return<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut ValueContext<'_, I>) -> Result<I::Effect, I::Error> {
        Ok(ForwardEffect::Return(
            ctx.read_many(self.values.as_slice())?,
        ))
    }
}
