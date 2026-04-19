use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_9::algebra::Lift;
use kirin_interpreter_9::control::{Control, CursorExt};
use kirin_interpreter_9::env::{AbstractEnv, AbstractMode, ConcreteMode, Env};
use kirin_interpreter_9::error::InterpreterError;
use kirin_interpreter_9::interpretable::Interpretable;

use crate::ForLoopValue;
use crate::interpreter9::cursor::{AbstractForCursor, AbstractIfCursor, ForCursor, IfCursor};

use crate::{For, If, Yield};

// ---------------------------------------------------------------------------
// If concrete helper — called by composed language Interpretable impls
// ---------------------------------------------------------------------------

/// Evaluate `scf.if` in concrete mode, creating an `IfCursor<V, L>` lifted to `C`.
///
/// Call this from your composed language's `Interpretable<ConcreteInterp<...>> for HighLevel`
/// impl, passing the concrete language type as `L`.
pub fn eval_if_concrete<E, C, L, T>(
    op: &If<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: Env<Mode = ConcreteMode<C>, Ext = CursorExt<C>>,
    E::Value: Clone + BranchCondition + ProductValue + 'static,
    IfCursor<E::Value, L>: Lift<C>,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    let cond = env.read(op.condition)?;
    let block = match cond.is_truthy() {
        Some(true) => op.then_body,
        Some(false) => op.else_body,
        None => {
            return Err(E::Error::from(InterpreterError::UnhandledEffect(
                "scf.if: nondeterministic condition in concrete mode".into(),
            )));
        }
    };
    let cursor = IfCursor::<E::Value, L>::new(block, op.results.clone(), env.current_stage());
    Ok(Control::Ext(CursorExt::Push(cursor.lift())))
}

/// Evaluate `scf.if` in abstract mode, creating an `AbstractIfCursor<V, L>` lifted to `C`.
pub fn eval_if_abstract<E, C, L, T>(
    op: &If<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: AbstractEnv<Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    E::Value: Clone + ProductValue + AbstractValue + 'static,
    AbstractIfCursor<E::Value, L>: Lift<C>,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    let stage = env.current_stage();
    let cursor =
        AbstractIfCursor::<E::Value, L>::new(op.then_body, op.else_body, op.results.clone(), stage);
    Ok(Control::Ext(CursorExt::Push(cursor.lift())))
}

// ---------------------------------------------------------------------------
// For concrete helper
// ---------------------------------------------------------------------------

/// Evaluate `scf.for` in concrete mode.
pub fn eval_for_concrete<E, C, L, T>(
    op: &For<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: Env<Mode = ConcreteMode<C>, Ext = CursorExt<C>>,
    E::Value: Clone + ForLoopValue + ProductValue + 'static,
    ForCursor<E::Value, L>: Lift<C>,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    let iv = env.read(op.start)?;
    let end = env.read(op.end)?;
    let step = env.read(op.step)?;
    let init_values: Vec<E::Value> = op
        .init_args
        .iter()
        .map(|ssa| env.read(*ssa))
        .collect::<Result<_, _>>()?;
    let init_arg_count = init_values.len();
    let carried = E::Value::new_product(init_values);
    let body_stage = env.current_stage();
    let cursor = ForCursor::<E::Value, L>::new(
        iv,
        end,
        step,
        carried,
        op.body,
        body_stage,
        init_arg_count,
        op.results.clone(),
    );
    Ok(Control::Ext(CursorExt::Push(cursor.lift())))
}

/// Evaluate `scf.for` in abstract mode.
pub fn eval_for_abstract<E, C, L, T>(
    op: &For<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: AbstractEnv<Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    E::Value: Clone + ProductValue + AbstractValue + 'static,
    AbstractForCursor<E::Value, L>: Lift<C>,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    let init_values: Vec<E::Value> = op
        .init_args
        .iter()
        .map(|ssa| env.read(*ssa))
        .collect::<Result<_, _>>()?;
    let init_arg_count = init_values.len();
    let carried = E::Value::new_product(init_values);
    let body_stage = env.current_stage();
    let cursor = AbstractForCursor::<E::Value, L>::new(
        carried,
        op.body,
        body_stage,
        init_arg_count,
        op.results.clone(),
        10,
    );
    Ok(Control::Ext(CursorExt::Push(cursor.lift())))
}

// ---------------------------------------------------------------------------
// Yield — single generic impl (works for both modes)
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Yield<T>
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
        Ok(Control::Yield(product))
    }
}
