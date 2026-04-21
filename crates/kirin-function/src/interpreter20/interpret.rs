use kirin::prelude::CompileTimeValue;
use kirin_interpreter::ProductValue;
use kirin_interpreter_20::control::Control;
use kirin_interpreter_20::dispatch::Dispatch;
use kirin_interpreter_20::env::Env;
use kirin_interpreter_20::error::InterpreterError;
use kirin_interpreter_20::interpretable::Interpretable;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// Structural ops
// ---------------------------------------------------------------------------

macro_rules! structural_error_impl {
    ($Op:ident, $msg:literal) => {
        impl<E, T> Interpretable<E> for $Op<T>
        where
            E: Env,
            E::Error: From<InterpreterError>,
            T: CompileTimeValue,
        {
            fn eval(&self, _env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
                Err(E::Error::from(InterpreterError::UnhandledEffect(
                    $msg.into(),
                )))
            }
        }
    };
}

structural_error_impl!(
    FunctionBody,
    "function bodies are structural and should not be stepped directly"
);
structural_error_impl!(Bind, "bind is not yet supported in interpreter19");
structural_error_impl!(
    Lambda,
    "lambda is structural and should not be stepped directly"
);

// ---------------------------------------------------------------------------
// Return
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Return<T>
where
    E: Env,
    E::Value: Clone + ProductValue,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let values: Vec<E::Value> = self
            .values
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = E::Value::new_product(values);
        Ok(Control::Return(product))
    }
}

// ---------------------------------------------------------------------------
// Call — dispatches via Dispatch::dispatch_call (no dialect-type parameter).
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Call<T>
where
    E: Dispatch,
    E::Value: Clone,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let args = env.read_many(self.args())?;
        let stage = env.current_stage();
        env.dispatch_call(self.target(), stage, args, self.results().to_vec())
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates all arms
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lexical<T>
where
    E: Dispatch,
    E::Value: Clone + ProductValue,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
            Lexical::FunctionBody(op) => op.eval(env),
            Lexical::Lambda(op) => op.eval(env),
            Lexical::Call(op) => op.eval(env),
            Lexical::Return(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Lifted
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lifted<T>
where
    E: Dispatch,
    E::Value: Clone + ProductValue,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
            Lifted::FunctionBody(op) => op.eval(env),
            Lifted::Bind(op) => op.eval(env),
            Lifted::Call(op) => op.eval(env),
            Lifted::Return(op) => op.eval(env),
        }
    }
}
