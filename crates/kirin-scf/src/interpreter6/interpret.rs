use kirin::prelude::CompileTimeValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_6::concrete::ConcreteDomain;
use kirin_interpreter_6::core::Core;
use kirin_interpreter_6::cursor::BlockCursor;
use kirin_interpreter_6::env::Interpretable;
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::{Lift, Project};

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

use super::cursor::{ForCursor, IfCursor, SCFCursor};

// ---------------------------------------------------------------------------
// If
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for If<T>
where
    T: CompileTimeValue,
    V: Clone + BranchCondition + 'static,
    E: ConcreteDomain<Value = V>,
    E::Cursor: Lift<BlockCursor<V, E::Language>> + Lift<IfCursor<V, E::Language>>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    /// Op-level effect: directly Core (push the IfCursor).
    type DialectEffect = Core<V, E::Cursor>;

    fn interpret(&self, env: &mut E) -> Result<Core<V, E::Cursor>, E::Error> {
        let cond = env.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(E::Error::from(InterpreterError::UnhandledEffect(
                    "scf.if: nondeterministic condition not supported".into(),
                )));
            }
        };
        let cursor =
            IfCursor::<V, E::Language>::new(block, self.results.clone(), env.current_stage());
        Ok(Core::Push(E::Cursor::lift(cursor)))
    }
}

// ---------------------------------------------------------------------------
// For
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for For<T>
where
    T: CompileTimeValue,
    V: Clone + ForLoopValue + ProductValue + 'static,
    E: ConcreteDomain<Value = V>,
    E::Cursor: Lift<BlockCursor<V, E::Language>> + Lift<ForCursor<V, E::Language>>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = Core<V, E::Cursor>;

    fn interpret(&self, env: &mut E) -> Result<Core<V, E::Cursor>, E::Error> {
        let iv = env.read(self.start)?;
        let end = env.read(self.end)?;
        let step = env.read(self.step)?;
        let init_values: Vec<V> = self
            .init_args
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let init_arg_count = init_values.len();
        let carried = V::new_product(init_values);
        let cursor = ForCursor::<V, E::Language>::builder()
            .iv(iv)
            .end(end)
            .step(step)
            .carried(carried)
            .body(self.body)
            .body_stage(env.current_stage())
            .init_arg_count(init_arg_count)
            .results(self.results.clone())
            .build();
        Ok(Core::Push(E::Cursor::lift(cursor)))
    }
}

// ---------------------------------------------------------------------------
// Yield
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for Yield<T>
where
    T: CompileTimeValue,
    V: Clone + ProductValue,
    E: ConcreteDomain<Value = V>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = Core<V, E::Cursor>;

    fn interpret(&self, env: &mut E) -> Result<Core<V, E::Cursor>, E::Error> {
        let values: Vec<V> = self
            .values
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = V::new_product(values);
        Ok(Core::Yield(product))
    }
}

// ---------------------------------------------------------------------------
// StructuredControlFlow — dialect wrapper; lifts op-level Core into E::Effect
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for StructuredControlFlow<T>
where
    T: CompileTimeValue,
    V: Clone + BranchCondition + ForLoopValue + ProductValue + 'static,
    E: ConcreteDomain<Value = V>,
    E::Cursor: Lift<BlockCursor<V, E::Language>>
        + Lift<IfCursor<V, E::Language>>
        + Lift<ForCursor<V, E::Language>>
        + Lift<SCFCursor<V, E::Language>>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    /// Dialect-wrapper level: already lifted to E::Effect.
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            Self::If(op) => op.interpret(env).map(E::Effect::lift),
            Self::For(op) => op.interpret(env).map(E::Effect::lift),
            Self::Yield(op) => op.interpret(env).map(E::Effect::lift),
        }
    }
}
