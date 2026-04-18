use std::convert::Infallible;

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{AbstractValue, ProductValue};
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::control::{Control, ControlExt};
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::interp::Interp;
use kirin_interpreter_7::store::Store;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// Structural-error impls: FunctionBody, Bind, Lambda
//
// These are structural ops that should never be stepped directly.
// Both concrete and abstract modes return an error.
// ---------------------------------------------------------------------------

macro_rules! structural_error_impl {
    ($Op:ident, $msg:literal) => {
        impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for $Op<T>
        where
            S: StageMeta + HasStageInfo<L>,
            L: Dialect,
            V: Clone,
            C: 'static,
            T: CompileTimeValue,
        {
            type Effect = Control<V, ControlExt<C>>;

            fn interpret(
                &self,
                _env: &mut ConcreteInterp<'ir, S, L, V, C>,
            ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
                Err(InterpreterError::UnhandledEffect($msg.into()))
            }
        }

        impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for $Op<T>
        where
            S: StageMeta + HasStageInfo<L>,
            L: Dialect,
            V: Clone + AbstractValue,
            T: CompileTimeValue,
        {
            type Effect = Control<V, Infallible>;

            fn interpret(
                &self,
                _env: &mut AbstractInterp<'ir, S, L, V>,
            ) -> Result<Control<V, Infallible>, InterpreterError> {
                Err(InterpreterError::UnhandledEffect($msg.into()))
            }
        }
    };
}

structural_error_impl!(
    FunctionBody,
    "function bodies are structural and should not be stepped directly"
);
structural_error_impl!(Bind, "bind is not yet supported in interpreter7");
structural_error_impl!(
    Lambda,
    "lambda is structural and should not be stepped directly"
);

// ---------------------------------------------------------------------------
// Return
// ---------------------------------------------------------------------------

fn interpret_return<S, T, Ext>(
    op: &Return<T>,
    env: &mut S,
) -> Result<Control<S::Value, Ext>, S::Error>
where
    S: Store,
    S::Value: Clone + ProductValue,
    T: CompileTimeValue,
{
    let values: Vec<S::Value> = op
        .values
        .iter()
        .map(|ssa| env.read(*ssa))
        .collect::<Result<_, _>>()?;
    let product = S::Value::new_product(values);
    Ok(Control::Return(product))
}

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Return<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        interpret_return(self, env)
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Return<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn interpret(
        &self,
        env: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        interpret_return(self, env)
    }
}

// ---------------------------------------------------------------------------
// Call
// ---------------------------------------------------------------------------

fn interpret_call<E, T, Ext>(op: &Call<T>, env: &mut E) -> Result<Control<E::Value, Ext>, E::Error>
where
    E: Interp,
    E::Value: Clone,
    E::StageContainer: HasStageInfo<E::Dialect>,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    let args = env.read_many(op.args())?;
    let stage_id = env.current_stage();
    let callee = env.resolve_function(op.target(), stage_id)?;
    Ok(Control::Call {
        callee,
        stage: stage_id,
        args,
        results: op.results().to_vec(),
    })
}

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Call<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        interpret_call(self, env)
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Call<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn interpret(
        &self,
        env: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        interpret_call(self, env)
    }
}

// ---------------------------------------------------------------------------
// Lifted — delegates to inner types
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Lifted<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(env),
            Lifted::Bind(op) => op.interpret(env),
            Lifted::Call(op) => op.interpret(env),
            Lifted::Return(op) => op.interpret(env),
        }
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Lifted<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn interpret(
        &self,
        env: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        match self {
            Lifted::FunctionBody(op) => op.interpret(env),
            Lifted::Bind(op) => op.interpret(env),
            Lifted::Call(op) => op.interpret(env),
            Lifted::Return(op) => op.interpret(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates to inner types
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Lexical<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        match self {
            Lexical::FunctionBody(op) => op.interpret(env),
            Lexical::Lambda(op) => op.interpret(env),
            Lexical::Call(op) => op.interpret(env),
            Lexical::Return(op) => op.interpret(env),
        }
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for Lexical<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn interpret(
        &self,
        env: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        match self {
            Lexical::FunctionBody(op) => op.interpret(env),
            Lexical::Lambda(op) => op.interpret(env),
            Lexical::Call(op) => op.interpret(env),
            Lexical::Return(op) => op.interpret(env),
        }
    }
}
