use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_19::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_19::abstract_interp::AbstractInterp;
use kirin_interpreter_19::algebra::Lift;
use kirin_interpreter_19::concrete::ConcreteInterp;
use kirin_interpreter_19::control::{Control, CursorExt};
use kirin_interpreter_19::cursor::BlockCursor;
use kirin_interpreter_19::env::Env;
use kirin_interpreter_19::error::InterpreterError;
use kirin_interpreter_19::interpretable::Interpretable;

use crate::ForLoopValue;
use crate::interpreter19::cursor::{AbstractForCursor, AbstractIfCursor, ForCursor, IfCursor};
use crate::{For, If, StructuredControlFlow, Yield};

// ---------------------------------------------------------------------------
// ScfSeam — crate-private, parametric over the dialect TYPE not the dialect.
// ---------------------------------------------------------------------------

pub trait ScfSeam<T: CompileTimeValue>: Env {
    fn eval_if(&mut self, op: &If<T>) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
    fn eval_for(&mut self, op: &For<T>) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
}

// ---------------------------------------------------------------------------
// Interpretable impls for If, For, StructuredControlFlow
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for If<T>
where
    T: CompileTimeValue,
    E: ScfSeam<T>,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        env.eval_if(self)
    }
}

impl<E, T> Interpretable<E> for For<T>
where
    T: CompileTimeValue,
    E: ScfSeam<T>,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        env.eval_for(self)
    }
}

impl<E, T> Interpretable<E> for StructuredControlFlow<T>
where
    T: CompileTimeValue,
    E: Env,
    If<T>: Interpretable<E>,
    For<T>: Interpretable<E>,
    Yield<T>: Interpretable<E>,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
            StructuredControlFlow::If(op) => op.eval(env),
            StructuredControlFlow::For(op) => op.eval(env),
            StructuredControlFlow::Yield(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Blanket ScfSeam impl for ConcreteInterp
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> ScfSeam<L::Type> for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition + ProductValue + ForLoopValue,
    IfCursor<V, L>: Lift<C>,
    ForCursor<V, L>: Lift<C>,
    BlockCursor<V, L>: Lift<C>,
    Self: Env<Ext = CursorExt<C>, Value = V>,
    Self::Error: From<InterpreterError>,
{
    fn eval_if(&mut self, op: &If<L::Type>) -> Result<Control<V, CursorExt<C>>, Self::Error> {
        eval_if_concrete::<_, C, L, _>(op, self)
    }

    fn eval_for(&mut self, op: &For<L::Type>) -> Result<Control<V, CursorExt<C>>, Self::Error> {
        eval_for_concrete::<_, C, L, _>(op, self)
    }
}

// ---------------------------------------------------------------------------
// Blanket ScfSeam impl for AbstractInterp
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> ScfSeam<L::Type> for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue + ForLoopValue,
    AbstractIfCursor<V, L>: Lift<C>,
    AbstractForCursor<V, L>: Lift<C>,
    BlockCursor<V, L>: Lift<C>,
    Self: Env<Ext = CursorExt<C>, Value = V>,
    Self::Error: From<InterpreterError>,
{
    fn eval_if(&mut self, op: &If<L::Type>) -> Result<Control<V, CursorExt<C>>, Self::Error> {
        eval_if_abstract::<_, C, L, _>(op, self)
    }

    fn eval_for(&mut self, op: &For<L::Type>) -> Result<Control<V, CursorExt<C>>, Self::Error> {
        eval_for_abstract::<_, C, L, _>(op, self)
    }
}

// ---------------------------------------------------------------------------
// If concrete helper
// ---------------------------------------------------------------------------

pub(crate) fn eval_if_concrete<E, C, L, T>(
    op: &If<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: Env<Ext = CursorExt<C>>,
    E::Value: Clone + BranchCondition + ProductValue,
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

pub(crate) fn eval_if_abstract<E, C, L, T>(
    op: &If<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: Env<Ext = CursorExt<C>>,
    E::Value: Clone + ProductValue + AbstractValue,
    BlockCursor<E::Value, L>: Lift<C>,
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

pub(crate) fn eval_for_concrete<E, C, L, T>(
    op: &For<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: Env<Ext = CursorExt<C>>,
    E::Value: Clone + ForLoopValue + ProductValue,
    ForCursor<E::Value, L>: Lift<C>,
    BlockCursor<E::Value, L>: Lift<C>,
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

pub(crate) fn eval_for_abstract<E, C, L, T>(
    op: &For<T>,
    env: &mut E,
) -> Result<Control<E::Value, CursorExt<C>>, E::Error>
where
    L: Dialect,
    E: Env<Ext = CursorExt<C>>,
    E::Value: Clone + ProductValue + AbstractValue,
    BlockCursor<E::Value, L>: Lift<C>,
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
    let max_iter = 10usize;
    let cursor = AbstractForCursor::<E::Value, L>::new(
        carried,
        op.body,
        body_stage,
        init_arg_count,
        op.results.clone(),
        max_iter,
    );
    Ok(Control::Ext(CursorExt::Push(cursor.lift())))
}

// ---------------------------------------------------------------------------
// Yield
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
