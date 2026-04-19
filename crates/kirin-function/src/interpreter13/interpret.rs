use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{AbstractValue, ProductValue};
use kirin_interpreter_13::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_13::abstract_interp::AbstractInterp;
use kirin_interpreter_13::algebra::SingleStageCursorFor;
use kirin_interpreter_13::concrete::ConcreteInterp;
use kirin_interpreter_13::control::Control;
use kirin_interpreter_13::env::Env;
use kirin_interpreter_13::error::InterpreterError;
use kirin_interpreter_13::interpretable::Interpretable;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// CallSeam — interpreter-side trait for call dispatch
//
// Blanket impls for ConcreteInterp and AbstractInterp are now provided here,
// gated on `C: SingleStageCursorFor<L>`. Multi-stage interpreters use cursor
// types that do NOT implement `SingleStageCursorFor<L>`, so they can provide
// their own specific `CallSeam<L>` impl without coherence conflicts.
// ---------------------------------------------------------------------------

pub trait CallSeam<L: Dialect>: Env {
    fn eval_call(
        &mut self,
        op: &Call<L::Type>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
}

// ---------------------------------------------------------------------------
// Blanket CallSeam impl for ConcreteInterp — single-stage case.
//
// Gated on `C: SingleStageCursorFor<L>` to exclude multi-stage cursors.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> CallSeam<L> for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: SingleStageCursorFor<L>,
    Self::Error: From<InterpreterError>,
{
    fn eval_call(
        &mut self,
        op: &Call<L::Type>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error> {
        eval_call_for_dialect::<_, L, _>(op, self)
    }
}

// ---------------------------------------------------------------------------
// Blanket CallSeam impl for AbstractInterp — single-stage case.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> CallSeam<L> for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue,
    C: SingleStageCursorFor<L>,
    Self::Error: From<InterpreterError>,
{
    fn eval_call(
        &mut self,
        op: &Call<L::Type>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error> {
        eval_call_for_dialect::<_, L, _>(op, self)
    }
}

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
structural_error_impl!(Bind, "bind is not yet supported in interpreter13");
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
// Call — base impl errors; composed languages use CallSeam::eval_call.
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
// Dialect-aware Call helper
// ---------------------------------------------------------------------------

pub fn eval_call_for_dialect<E, L, T>(
    op: &Call<T>,
    env: &mut E,
) -> Result<Control<E::Value, E::Ext>, E::Error>
where
    E: Env,
    E::Stages: HasStageInfo<L>,
    E::Value: Clone,
    E::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue,
{
    let args = env.read_many(op.args())?;
    let stage_id = env.current_stage();
    let callee = env.resolve_function_for::<L>(op.target(), stage_id)?;
    Ok(Control::Call {
        callee,
        stage: stage_id,
        args,
        results: op.results().to_vec(),
    })
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

// ---------------------------------------------------------------------------
// Lexical
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
