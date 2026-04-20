use kirin::prelude::CompileTimeValue;
use kirin_interpreter::ProductValue;
use kirin_interpreter_17::control::Control;
use kirin_interpreter_17::env::Env;
use kirin_interpreter_17::error::InterpreterError;
use kirin_interpreter_17::interpretable::Interpretable;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// Note: CallSeam<L> and its blanket impls for ConcreteInterp / AbstractInterp
// live entirely in kirin-interpreter-17/src/call_seam.rs. Dialect impls that
// need call dispatch import kirin_interpreter_17::call_seam::CallSeam and call
// env.eval_call(target, stage, args, results) directly.
// ---------------------------------------------------------------------------

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
structural_error_impl!(Bind, "bind is not yet supported in interpreter17");
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
// Call — base impl errors; dialect impls invoke CallSeam::eval_call directly.
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Call<T>
where
    E: Env,
    E::Value: Clone,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, _env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "Call must be handled via CallSeam::eval_call".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates all arms; Call arm returns error (use CallSeam instead)
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lexical<T>
where
    E: Env,
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
    E: Env,
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
