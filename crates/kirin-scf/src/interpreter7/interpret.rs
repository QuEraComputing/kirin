use kirin::prelude::CompileTimeValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_7::control::{Control, ControlExt};
use kirin_interpreter_7::cursor::BlockCursor;
use kirin_interpreter_7::env::{ConcreteEnv, Interpretable};
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::lift::Lift;

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

use super::cursor::{ForCursor, IfCursor, SCFCursor};

// ---------------------------------------------------------------------------
// If — concrete mode
//
// Note: Abstract SCF interpretation is not supported in interpreter-7 via
// Interpretable<AbstractEnv>. The Rust coherence rules prevent having two
// impl blocks for Interpretable<E> on the same type (one for ConcreteEnv,
// one for AbstractEnv) because Rust can't prove the bounds are disjoint.
//
// For abstract interpretation, use LowLevel (flat CF) programs, which work
// with AbstractInterp via the Interpretable<E: Interp> impls in kirin-cf.
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for If<T>
where
    T: CompileTimeValue,
    V: Clone + BranchCondition + 'static,
    E: ConcreteEnv<Value = V>,
    E::Cursor: Lift<BlockCursor<V, E::Dialect>> + Lift<IfCursor<V, E::Dialect>>,
    E::Ext: From<ControlExt<E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type Effect = Control<V, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        let cond = env.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(E::Error::from(InterpreterError::UnhandledEffect(
                    "scf.if: nondeterministic condition not supported in concrete mode".into(),
                )));
            }
        };
        let cursor =
            IfCursor::<V, E::Dialect>::new(block, self.results.clone(), env.current_stage());
        Ok(Control::Ext(E::Ext::from(ControlExt::Push(
            E::Cursor::lift(cursor),
        ))))
    }
}

// ---------------------------------------------------------------------------
// For — concrete mode
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for For<T>
where
    T: CompileTimeValue,
    V: Clone + ForLoopValue + ProductValue + 'static,
    E: ConcreteEnv<Value = V>,
    E::Cursor: Lift<BlockCursor<V, E::Dialect>> + Lift<ForCursor<V, E::Dialect>>,
    E::Ext: From<ControlExt<E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type Effect = Control<V, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
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
        let cursor = ForCursor::<V, E::Dialect>::builder()
            .iv(iv)
            .end(end)
            .step(step)
            .carried(carried)
            .body(self.body)
            .body_stage(env.current_stage())
            .init_arg_count(init_arg_count)
            .results(self.results.clone())
            .build();
        Ok(Control::Ext(E::Ext::from(ControlExt::Push(
            E::Cursor::lift(cursor),
        ))))
    }
}

// ---------------------------------------------------------------------------
// Yield — concrete mode
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for Yield<T>
where
    T: CompileTimeValue,
    V: Clone + ProductValue,
    E: ConcreteEnv<Value = V>,
    E::Error: From<InterpreterError>,
{
    type Effect = Control<V, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        let values: Vec<V> = self
            .values
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = V::new_product(values);
        Ok(Control::Yield(product))
    }
}

// ---------------------------------------------------------------------------
// StructuredControlFlow — concrete dialect wrapper
// ---------------------------------------------------------------------------

impl<E, V, T> Interpretable<E> for StructuredControlFlow<T>
where
    T: CompileTimeValue,
    V: Clone + BranchCondition + ForLoopValue + ProductValue + 'static,
    E: ConcreteEnv<Value = V>,
    E::Cursor: Lift<BlockCursor<V, E::Dialect>>
        + Lift<IfCursor<V, E::Dialect>>
        + Lift<ForCursor<V, E::Dialect>>
        + Lift<SCFCursor<V, E::Dialect>>,
    E::Ext: From<ControlExt<E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    type Effect = Control<V, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            Self::If(op) => op.interpret(env),
            Self::For(op) => op.interpret(env),
            Self::Yield(op) => op.interpret(env),
        }
    }
}
