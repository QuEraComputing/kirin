use kirin::prelude::CompileTimeValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_5::concrete::ConcreteDomain;
use kirin_interpreter_5::cursor::{Boxed, Execute};
use kirin_interpreter_5::effect::ControlFlow;
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

use super::cursor::{ForCursor, IfCursor};

// ---------------------------------------------------------------------------
// If
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for If<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + BranchCondition + ProductValue + 'static,
    D::Error: From<InterpreterError>,
    IfCursor<V>: Execute<D>,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        let cond = domain.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(D::Error::from(InterpreterError::UnhandledEffect(
                    "scf.if: nondeterministic conditions not supported in interpreter5".into(),
                )));
            }
        };
        let cursor = IfCursor::new(block, self.results.clone(), domain.current_stage());
        Ok(ControlFlow::Push(Boxed(Box::new(cursor))))
    }
}

// ---------------------------------------------------------------------------
// For
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for For<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + ForLoopValue + ProductValue + 'static,
    D::Error: From<InterpreterError>,
    ForCursor<V>: Execute<D>,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        let iv = domain.read(self.start)?;
        let end = domain.read(self.end)?;
        let step = domain.read(self.step)?;
        let init_values: Vec<V> = self
            .init_args
            .iter()
            .map(|ssa| domain.read(*ssa))
            .collect::<Result<_, _>>()?;
        let init_arg_count = init_values.len();
        let carried = V::new_product(init_values);
        let cursor = ForCursor::new(
            iv,
            end,
            step,
            carried,
            self.body,
            init_arg_count,
            self.results.clone(),
            domain.current_stage(),
        );
        Ok(ControlFlow::Push(Boxed(Box::new(cursor))))
    }
}

// ---------------------------------------------------------------------------
// Yield — requires ConcreteDomain for ControlFlow::Yield
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for Yield<T>
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
        Ok(ControlFlow::Yield(product))
    }
}

// ---------------------------------------------------------------------------
// StructuredControlFlow — dispatches to inner types
// ---------------------------------------------------------------------------

impl<D, V, T> Interpretable<D> for StructuredControlFlow<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + BranchCondition + ForLoopValue + ProductValue + 'static,
    D::Error: From<InterpreterError>,
    IfCursor<V>: Execute<D>,
    ForCursor<V>: Execute<D>,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            Self::If(op) => op.interpret(domain),
            Self::For(op) => op.interpret(domain),
            Self::Yield(op) => op.interpret(domain),
        }
    }
}
