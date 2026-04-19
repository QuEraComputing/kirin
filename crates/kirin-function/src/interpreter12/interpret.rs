use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo};
use kirin_interpreter::ProductValue;
use kirin_interpreter_12::control::Control;
use kirin_interpreter_12::env::Env;
use kirin_interpreter_12::error::InterpreterError;
use kirin_interpreter_12::interpretable::Interpretable;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// CallSeam — interpreter-side trait for call dispatch
//
// Dialect authors call `env.eval_call(op)` rather than directly invoking
// `eval_call_for_dialect`.
//
// NOTE: No blanket impls are provided here to avoid coherence conflicts with
// user-defined multi-stage impls. User code (e.g. toy-lang) provides specific
// impls for each interpreter type.
// ---------------------------------------------------------------------------

/// Mode-agnostic call dispatch. Implemented by interpreter types.
///
/// User code provides specific `CallSeam<L>` impls for each interpreter type
/// (single-stage: delegates to `eval_call_for_dialect`; multi-stage: custom).
pub trait CallSeam<L: Dialect>: Env {
    fn eval_call(
        &mut self,
        op: &Call<L::Type>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
}

// ---------------------------------------------------------------------------
// Structural ops: FunctionBody, Bind, Lambda
// These are container ops and should never be stepped directly.
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
structural_error_impl!(Bind, "bind is not yet supported in interpreter12");
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
// Call — requires dialect context to resolve the callee.
// The base impl errors; composed languages use CallSeam::eval_call instead.
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Call<T>
where
    E: Env,
    E::Value: Clone,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let _ = env;
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "Call must be handled via CallSeam::eval_call".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Dialect-aware Call helper — single-stage callers use this.
// ---------------------------------------------------------------------------

/// Resolve and emit a `Control::Call` for a `Call<T>` op using dialect `L`.
///
/// Used by the `CallSeam` blanket impls and by user-code multi-stage impls.
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
// Lifted — delegates to inner types (all generic)
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
// Lexical — delegates to inner types (all generic)
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
