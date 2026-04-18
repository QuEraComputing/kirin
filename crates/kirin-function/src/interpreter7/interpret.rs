use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::ProductValue;
use kirin_interpreter_7::control::Control;
use kirin_interpreter_7::env::{Interp, Interpretable};
use kirin_interpreter_7::error::InterpreterError;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// FunctionBody — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for FunctionBody<T>
where
    E: Interp,
    T: CompileTimeValue,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, _env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "function bodies are structural and should not be stepped directly".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Bind — not yet supported
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Bind<T>
where
    E: Interp,
    T: CompileTimeValue,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, _env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "bind is not yet supported in interpreter7".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Lambda — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lambda<T>
where
    E: Interp,
    T: CompileTimeValue,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, _env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "lambda is structural and should not be stepped directly".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Return
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Return<T>
where
    T: CompileTimeValue,
    E: Interp,
    E::Value: Clone + ProductValue,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
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
// Call
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Call<T>
where
    T: CompileTimeValue,
    E: Interp,
    E::Value: Clone,
    E::StageContainer: HasStageInfo<E::Dialect>,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let args = env.read_many(self.args())?;
        let stage_id = env.current_stage();
        let callee = env.resolve_function(self.target(), stage_id)?;
        Ok(Control::Call {
            callee,
            stage: stage_id,
            args,
            results: self.results().to_vec(),
        })
    }
}

// ---------------------------------------------------------------------------
// Lifted — delegates to inner types
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lifted<T>
where
    T: CompileTimeValue,
    E: Interp,
    E::Value: Clone + ProductValue,
    E::StageContainer: HasStageInfo<E::Dialect>,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(env),
            Lifted::Bind(op) => op.interpret(env),
            Lifted::Call(op) => op.interpret(env),
            Lifted::Return(op) => op.interpret(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates to inner types
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lexical<T>
where
    T: CompileTimeValue,
    E: Interp,
    E::Value: Clone + ProductValue,
    E::StageContainer: HasStageInfo<E::Dialect>,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
            Lexical::FunctionBody(op) => op.interpret(env),
            Lexical::Lambda(op) => op.interpret(env),
            Lexical::Call(op) => op.interpret(env),
            Lexical::Return(op) => op.interpret(env),
        }
    }
}
