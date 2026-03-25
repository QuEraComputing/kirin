use kirin::prelude::CompileTimeValue;
use kirin_interpreter_2::{Interpretable, Interpreter, InterpreterError, ProductValue, ValueStore};

use crate::{Bind, Call, FunctionBody, Lifted, Return};

use super::{CallFrame, Effect, runtime::Runtime};

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for FunctionBody<T>
where
    I: Runtime<'ir, T>,
    T: CompileTimeValue,
    <I as ValueStore>::Value: Clone + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine<<I as ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Effect<<I as ValueStore>::Value>, Self::Error> {
        Err(unsupported("function bodies are structural and should not be stepped directly").into())
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Bind<T>
where
    I: Runtime<'ir, T>,
    T: CompileTimeValue,
    <I as ValueStore>::Value: Clone + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine<<I as ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Effect<<I as ValueStore>::Value>, Self::Error> {
        Err(unsupported("bind is not yet supported in interpreter2").into())
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Call<T>
where
    I: Runtime<'ir, T>,
    T: CompileTimeValue,
    <I as ValueStore>::Value: Clone + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine<<I as ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Effect<<I as ValueStore>::Value>, Self::Error> {
        let callee = interp.resolve_callee(self.target())?;
        let entry = interp.entry_block(callee)?;
        let args = interp.read_many(self.args())?;
        let resume = interp.resume_seed_after_current()?;
        let caller_bindings = interp.replace_value_bindings(Vec::new());

        if let Err(error) = interp.bind_function_args(entry, &args) {
            interp.replace_value_bindings(caller_bindings);
            return Err(error);
        }

        interp.function_machine_mut().push_frame(CallFrame::new(
            caller_bindings,
            self.results().to_vec(),
            resume,
        ));

        Ok(Effect::Jump(entry.into()))
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Return<T>
where
    I: Runtime<'ir, T>,
    T: CompileTimeValue,
    <I as ValueStore>::Value: Clone + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine<<I as ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Effect<<I as ValueStore>::Value>, Self::Error> {
        let product = <I as ValueStore>::Value::new_product(interp.read_many(&self.values)?);
        let Some(frame) = interp.function_machine_mut().pop_frame() else {
            return Ok(Effect::Stop(product));
        };
        let (caller_bindings, results, resume) = frame.into_parts();
        let callee_bindings = interp.replace_value_bindings(caller_bindings);

        if let Err(error) = interp.write_product(&results, product) {
            let caller_bindings = interp.replace_value_bindings(callee_bindings);
            interp.function_machine_mut().push_frame(CallFrame::new(
                caller_bindings,
                results,
                resume,
            ));
            return Err(error);
        }

        Ok(Effect::Jump(resume))
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Lifted<T>
where
    I: Runtime<'ir, T>,
    T: CompileTimeValue,
    <I as ValueStore>::Value: Clone + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    type Machine = super::Machine<<I as ValueStore>::Value>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Effect<<I as ValueStore>::Value>, Self::Error> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(interp),
            Lifted::Bind(op) => op.interpret(interp),
            Lifted::Call(op) => op.interpret(interp),
            Lifted::Return(op) => op.interpret(interp),
        }
    }
}
