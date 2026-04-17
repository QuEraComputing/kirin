use kirin::prelude::CompileTimeValue;
use kirin_interpreter::ProductValue;
use kirin_interpreter_5::concrete::ConcreteDomain;
use kirin_interpreter_5::cursor::Boxed;
use kirin_interpreter_5::effect::ControlFlow;
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// FunctionBody — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<D, T> Interpretable<D> for FunctionBody<T>
where
    D: Env,
    T: CompileTimeValue,
{
    fn interpret(&self, _domain: &mut D) -> Result<D::Effect, D::Error> {
        Err(D::Error::from(InterpreterError::UnhandledEffect(
            "function bodies are structural and should not be stepped directly".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Bind — not yet supported
// ---------------------------------------------------------------------------

impl<D, T> Interpretable<D> for Bind<T>
where
    D: Env,
    T: CompileTimeValue,
{
    fn interpret(&self, _domain: &mut D) -> Result<D::Effect, D::Error> {
        Err(D::Error::from(InterpreterError::UnhandledEffect(
            "bind is not yet supported in interpreter5".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Lambda — structural, should not be stepped directly
// ---------------------------------------------------------------------------

impl<D, T> Interpretable<D> for Lambda<T>
where
    D: Env,
    T: CompileTimeValue,
{
    fn interpret(&self, _domain: &mut D) -> Result<D::Effect, D::Error> {
        Err(D::Error::from(InterpreterError::UnhandledEffect(
            "lambda is structural and should not be stepped directly".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Return — requires ConcreteDomain
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for Return<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + ProductValue,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        let values: Vec<V> = self
            .values
            .iter()
            .map(|ssa| domain.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = V::new_product(values);
        Ok(ControlFlow::Return(product))
    }
}

// ---------------------------------------------------------------------------
// Call — requires ConcreteDomain to resolve functions
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for Call<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        let args = domain.read_many(self.args())?;
        let stage_id = domain.current_stage();
        let callee = domain.resolve_function(self.target(), stage_id)?;
        Ok(ControlFlow::Call {
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

impl<D, V, T> Interpretable<D> for Lifted<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + ProductValue,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(domain),
            Lifted::Bind(op) => op.interpret(domain),
            Lifted::Call(op) => op.interpret(domain),
            Lifted::Return(op) => op.interpret(domain),
        }
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates to inner types
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for Lexical<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + ProductValue,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            Lexical::FunctionBody(op) => op.interpret(domain),
            Lexical::Lambda(op) => op.interpret(domain),
            Lexical::Call(op) => op.interpret(domain),
            Lexical::Return(op) => op.interpret(domain),
        }
    }
}
