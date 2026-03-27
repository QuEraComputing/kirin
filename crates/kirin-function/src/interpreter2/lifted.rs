use kirin::prelude::CompileTimeValue;
use kirin_interpreter_2::{
    Interpretable, Interpreter, InterpreterError, ProductValue,
    effect::Flow,
    interpreter::{Invoke, ResolveCall, ResolveCallee},
};

use crate::{Bind, Call, FunctionBody, Lifted, Return};

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for FunctionBody<T>
where
    I: Interpreter<'ir>,
    T: CompileTimeValue,
{
    type Effect = Flow<<I as kirin_interpreter_2::ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(
        &self,
        _interp: &mut I,
    ) -> Result<Flow<<I as kirin_interpreter_2::ValueStore>::Value>, Self::Error> {
        Err(unsupported("function bodies are structural and should not be stepped directly").into())
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Bind<T>
where
    I: Interpreter<'ir>,
    T: CompileTimeValue,
{
    type Effect = Flow<<I as kirin_interpreter_2::ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(
        &self,
        _interp: &mut I,
    ) -> Result<Flow<<I as kirin_interpreter_2::ValueStore>::Value>, Self::Error> {
        Err(unsupported("bind is not yet supported in interpreter2").into())
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
        args: &[<I as kirin_interpreter_2::ValueStore>::Value],
    ) -> Result<kirin::prelude::SpecializedFunction, <I as Interpreter<'ir>>::Error> {
        interp.callee().symbol(self.target()).args(args)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Call<T>
where
    I: Invoke<'ir>
        + ResolveCallee<'ir>
        + kirin_interpreter_2::ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    T: CompileTimeValue,
    <I as kirin_interpreter_2::ValueStore>::Value: Clone,
{
    type Effect = Flow<<I as kirin_interpreter_2::ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Flow<<I as kirin_interpreter_2::ValueStore>::Value>, Self::Error> {
        let args = interp.read_many(self.args())?;
        let callee = self.resolve_call(interp, &args)?;
        interp.invoke(callee, &args, self.results())?;
        Ok(Flow::Stay)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Return<T>
where
    I: Invoke<'ir> + kirin_interpreter_2::ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    T: CompileTimeValue,
    <I as kirin_interpreter_2::ValueStore>::Value: Clone + ProductValue,
{
    type Effect = Flow<<I as kirin_interpreter_2::ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Flow<<I as kirin_interpreter_2::ValueStore>::Value>, Self::Error> {
        let product = <I as kirin_interpreter_2::ValueStore>::Value::new_product(
            interp.read_many(&self.values)?,
        );
        interp.return_current(product)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Lifted<T>
where
    I: Invoke<'ir>
        + ResolveCallee<'ir>
        + kirin_interpreter_2::ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    T: CompileTimeValue,
    <I as kirin_interpreter_2::ValueStore>::Value: Clone + ProductValue,
{
    type Effect = Flow<<I as kirin_interpreter_2::ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Flow<<I as kirin_interpreter_2::ValueStore>::Value>, Self::Error> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(interp),
            Lifted::Bind(op) => op.interpret(interp),
            Lifted::Call(op) => op.interpret(interp),
            Lifted::Return(op) => op.interpret(interp),
        }
    }
}
