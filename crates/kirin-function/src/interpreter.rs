use kirin::prelude::{CompileTimeValue, HasRegionBody, Product, SSAValue};
use kirin_interpreter::dialect::{
    CallEffect, Callee, ForwardEffect, ForwardEval, ForwardEvalInterp, FunctionBody, FunctionEntry,
    Interp, Interpretable, InterpreterError,
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
        _interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        Ok(FunctionBody::new(*self.region()).args(args))
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
        _interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        Ok(FunctionBody::new(*self.region()).args(args))
    }
}

/// Function definitions are inert at runtime: defining a function does not
/// execute its body. Bodies run when the function is invoked (via
/// [`FunctionEntry`]).
impl<I, T> Interpretable<I, ForwardEval> for Function<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(ForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Lambda<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "first-class lambda values are not yet supported",
        )))
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Bind<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "bind is not yet supported by the interpreter",
        )))
    }
}

fn call_effect<I, T, C>(call: &C, callee: Callee, interp: &mut I) -> Result<I::Effect, I::Error>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
    C: CallLike<T>,
{
    let args = call
        .arguments()
        .map(|argument| interp.read(*argument))
        .collect::<Result<Product<_>, _>>()?;
    let results = call.results().copied().map(SSAValue::from).collect();
    Ok(ForwardEffect::Call(CallEffect {
        callee,
        stage: call.stage(),
        args,
        results,
    }))
}

impl<I, T> Interpretable<I, ForwardEval> for CallNamed<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Named(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for CallFunction<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Function(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for CallStaged<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Staged(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for CallSpecialized<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Specialized(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Return<T>
where
    I: ForwardEvalInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(ForwardEffect::Return(
            interp.read_many(self.values.as_slice())?,
        ))
    }
}
