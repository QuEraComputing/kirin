use kirin::prelude::CompileTimeValue;
use kirin_interpreter::ProductValue;
use kirin_interpreter_6::abstract_domain::BaseDomain;
use kirin_interpreter_6::core::Core;
use kirin_interpreter_6::env::{Env, Interpretable};
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::{Lift, Project};

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// FunctionBody — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for FunctionBody<T>
where
    E: Env,
    T: CompileTimeValue,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, _env: &mut E) -> Result<E::Effect, E::Error> {
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
    E: Env,
    T: CompileTimeValue,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, _env: &mut E) -> Result<E::Effect, E::Error> {
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "bind is not yet supported in interpreter6".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Lambda — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lambda<T>
where
    E: Env,
    T: CompileTimeValue,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, _env: &mut E) -> Result<E::Effect, E::Error> {
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "lambda is structural and should not be stepped directly".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Return — requires BaseDomain
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Return<T>
where
    T: CompileTimeValue,
    E: BaseDomain,
    E::Value: Clone + ProductValue,
    // Restated from BaseDomain's where clause — Rust does not automatically
    // propagate trait where-clause bounds to generic users of the trait.
    E::Effect: Lift<Core<E::Value, E::Cursor>> + Project<Core<E::Value, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = Core<E::Value, E::Cursor>;

    fn interpret(&self, env: &mut E) -> Result<Core<E::Value, E::Cursor>, E::Error> {
        let values: Vec<E::Value> = self
            .values
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = E::Value::new_product(values);
        Ok(Core::Return(product))
    }
}

// ---------------------------------------------------------------------------
// Call — requires BaseDomain to resolve functions
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Call<T>
where
    T: CompileTimeValue,
    E: BaseDomain,
    E::Value: Clone,
    // Restated from BaseDomain's where clause.
    E::Effect: Lift<Core<E::Value, E::Cursor>> + Project<Core<E::Value, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = Core<E::Value, E::Cursor>;

    fn interpret(&self, env: &mut E) -> Result<Core<E::Value, E::Cursor>, E::Error> {
        let args = env.read_many(self.args())?;
        let stage_id = env.current_stage();
        let callee = env.resolve_function(self.target(), stage_id)?;
        Ok(Core::Call {
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
    E: BaseDomain,
    E::Value: Clone + ProductValue,
    // Restated from BaseDomain's where clause.
    E::Effect: Lift<Core<E::Value, E::Cursor>> + Project<Core<E::Value, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(env),
            Lifted::Bind(op) => op.interpret(env),
            Lifted::Call(op) => op.interpret(env).map(E::Effect::lift),
            Lifted::Return(op) => op.interpret(env).map(E::Effect::lift),
        }
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates to inner types
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Lexical<T>
where
    T: CompileTimeValue,
    E: BaseDomain,
    E::Value: Clone + ProductValue,
    // Restated from BaseDomain's where clause.
    E::Effect: Lift<Core<E::Value, E::Cursor>> + Project<Core<E::Value, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            Lexical::FunctionBody(op) => op.interpret(env),
            Lexical::Lambda(op) => op.interpret(env),
            Lexical::Call(op) => op.interpret(env).map(E::Effect::lift),
            Lexical::Return(op) => op.interpret(env).map(E::Effect::lift),
        }
    }
}
