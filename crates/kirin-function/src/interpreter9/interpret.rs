use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo};
use kirin_interpreter::ProductValue;
use kirin_interpreter_9::control::Control;
use kirin_interpreter_9::env::Env;
use kirin_interpreter_9::error::InterpreterError;
use kirin_interpreter_9::interpretable::Interpretable;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// Structural ops: FunctionBody, Bind, Lambda
// These should never be stepped directly.
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
structural_error_impl!(Bind, "bind is not yet supported in interpreter9");
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
// Call
// ---------------------------------------------------------------------------

impl<E, T> Interpretable<E> for Call<T>
where
    E: Env,
    E::Value: Clone,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let args = env.read_many(self.args())?;
        let stage_id = env.current_stage();
        // NOTE: Call requires dialect L to resolve the function. Since Interpretable<E>
        // doesn't carry L, we use a workaround: encode the call via the stage resolution
        // on the env. This is resolved at the composed language level.
        // For now, emit a structural error — composed languages override this.
        let _ = (args, stage_id);
        Err(E::Error::from(InterpreterError::UnhandledEffect(
            "Call must be handled by the composed language's Interpretable impl".into(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Dialect-aware Call helper — used by composed languages
// ---------------------------------------------------------------------------

/// Resolve and emit a `Control::Call` for a `Call<T>` op using dialect `L`.
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
// Lifted — delegates to inner types
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
// Lexical — delegates to inner types
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
