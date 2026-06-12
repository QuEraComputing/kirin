use kirin::prelude::{CompileTimeValue, HasRegionBody, Product, SSAValue};
use kirin_interpreter::dialect::{
    CallEffect, Callee, Ctx, Effect, FunctionEntry, Interp, Interpretable, InterpreterError, Scope,
};

use crate::{
    Bind, CallFunction, CallLike, CallNamed, CallSpecialized, CallStaged, Function, Lambda, Return,
};

impl<I, T> FunctionEntry<I> for Function<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn function_entry(
        &self,
        args: Product<I::Value>,
        _ctx: &mut Ctx<'_, I>,
    ) -> Result<Scope<I::Value, I::Error>, I::Error> {
        Ok(Scope::region(*self.region()).args(args))
    }
}

impl<I, T> FunctionEntry<I> for Lambda<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn function_entry(
        &self,
        args: Product<I::Value>,
        _ctx: &mut Ctx<'_, I>,
    ) -> Result<Scope<I::Value, I::Error>, I::Error> {
        Ok(Scope::region(*self.region()).args(args))
    }
}

/// Function definitions are inert at runtime: defining a function does not
/// execute its body. Bodies run when the function is invoked (via
/// [`FunctionEntry`]).
impl<I, T> Interpretable<I> for Function<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, _ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        Ok(Effect::Next)
    }
}

impl<I, T> Interpretable<I> for Lambda<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, _ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "first-class lambda values are not yet supported",
        )))
    }
}

impl<I, T> Interpretable<I> for Bind<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, _ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "bind is not yet supported by the interpreter",
        )))
    }
}

fn call_effect<I, T, C>(
    call: &C,
    callee: Callee,
    ctx: &mut Ctx<'_, I>,
) -> Result<Effect<I::Value, I::Error>, I::Error>
where
    I: Interp,
    T: CompileTimeValue,
    C: CallLike<T>,
{
    let args = call
        .arguments()
        .map(|argument| ctx.read(*argument))
        .collect::<Result<Product<_>, _>>()?;
    let results = call.results().copied().map(SSAValue::from).collect();
    Ok(Effect::Call(CallEffect {
        callee,
        stage: call.stage(),
        args,
        results,
    }))
}

impl<I, T> Interpretable<I> for CallNamed<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        call_effect(self, Callee::Named(self.target()), ctx)
    }
}

impl<I, T> Interpretable<I> for CallFunction<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        call_effect(self, Callee::Function(self.target()), ctx)
    }
}

impl<I, T> Interpretable<I> for CallStaged<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        call_effect(self, Callee::Staged(self.target()), ctx)
    }
}

impl<I, T> Interpretable<I> for CallSpecialized<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        call_effect(self, Callee::Specialized(self.target()), ctx)
    }
}

impl<I, T> Interpretable<I> for Return<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        Ok(Effect::Return(ctx.read_many(self.values.as_slice())?))
    }
}
