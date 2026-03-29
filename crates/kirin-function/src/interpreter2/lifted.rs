use kirin::prelude::CompileTimeValue;
use kirin_interpreter_2::{
    Interpretable, Interpreter, InterpreterError, Machine, ProductValue, ValueStore,
    control::Directive,
    interpreter::{Invoke, ResolveCall, ResolveCallee},
};

use crate::{Bind, Call, FunctionBody, Lifted, Return};

impl<'ir, I, T> Interpretable<'ir, I> for FunctionBody<T>
where
    I: Interpreter<'ir>,
    T: CompileTimeValue,
{
    type Effect = Directive<I::Value, <I as Machine<'ir>>::Seed>;
    type Error = <I as ValueStore>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Self::Effect, Self::Error> {
        Err(InterpreterError::unsupported(
            "function bodies are structural and should not be stepped directly",
        )
        .into())
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Bind<T>
where
    I: Interpreter<'ir>,
    T: CompileTimeValue,
{
    type Effect = Directive<I::Value, <I as Machine<'ir>>::Seed>;
    type Error = <I as ValueStore>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Self::Effect, Self::Error> {
        Err(InterpreterError::unsupported("bind is not yet supported in interpreter2").into())
    }
}

impl<'ir, I, T> ResolveCall<'ir, I> for Call<T>
where
    I: ResolveCallee<'ir>,
    T: CompileTimeValue,
{
    fn resolve_call(
        &self,
        interp: &I,
        args: &[I::Value],
    ) -> Result<kirin::prelude::SpecializedFunction, <I as ValueStore>::Error> {
        interp.callee().symbol(self.target()).args(args)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Call<T>
where
    I: Invoke<'ir> + ResolveCallee<'ir>,
    T: CompileTimeValue,
    I::Value: Clone,
{
    type Effect = Directive<I::Value, <I as Machine<'ir>>::Seed>;
    type Error = <I as ValueStore>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Self::Effect, Self::Error> {
        let args = interp.read_many(self.args())?;
        let callee = self.resolve_call(interp, &args)?;
        interp.invoke(callee, &args, self.results())?;
        Ok(Directive::Stay)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Return<T>
where
    I: Invoke<'ir>,
    T: CompileTimeValue,
    I::Value: Clone + ProductValue,
{
    type Effect = Directive<I::Value, <I as Machine<'ir>>::Seed>;
    type Error = <I as ValueStore>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Self::Effect, Self::Error> {
        let product = I::Value::new_product(interp.read_many(&self.values)?);
        interp.return_current(product)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Lifted<T>
where
    I: Invoke<'ir> + ResolveCallee<'ir>,
    T: CompileTimeValue,
    I::Value: Clone + ProductValue,
{
    type Effect = Directive<I::Value, <I as Machine<'ir>>::Seed>;
    type Error = <I as ValueStore>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Self::Effect, Self::Error> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(interp),
            Lifted::Bind(op) => op.interpret(interp),
            Lifted::Call(op) => op.interpret(interp),
            Lifted::Return(op) => op.interpret(interp),
        }
    }
}
