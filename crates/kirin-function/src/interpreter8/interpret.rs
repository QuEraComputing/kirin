use std::convert::Infallible;

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{AbstractValue, ProductValue};
use kirin_interpreter_8::abstract_interp::AbstractInterp;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::control::{Control, CursorExt};
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

// ---------------------------------------------------------------------------
// Structural-error impls: FunctionBody, Bind, Lambda
//
// These are structural ops that should never be stepped directly.
// ---------------------------------------------------------------------------

macro_rules! structural_error_impl {
    ($Op:ident, $msg:literal) => {
        impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for $Op<T>
        where
            S: StageMeta + HasStageInfo<L>,
            L: Dialect,
            V: Clone,
            C: 'static,
            T: CompileTimeValue,
        {
            type Effect = Control<V, CursorExt<C>>;

            fn eval(
                &self,
                _domain: &mut ConcreteInterp<'ir, S, L, V, C>,
            ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
                Err(InterpreterError::UnhandledEffect($msg.into()))
            }
        }

        impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for $Op<T>
        where
            S: StageMeta + HasStageInfo<L>,
            L: Dialect,
            V: Clone + AbstractValue,
            T: CompileTimeValue,
        {
            type Effect = Control<V, Infallible>;

            fn eval(
                &self,
                _domain: &mut AbstractInterp<'ir, S, L, V>,
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
structural_error_impl!(Bind, "bind is not yet supported in interpreter8");
structural_error_impl!(
    Lambda,
    "lambda is structural and should not be stepped directly"
);

// ---------------------------------------------------------------------------
// Return
// ---------------------------------------------------------------------------

fn eval_return<D, T, Ext>(
    op: &Return<T>,
    domain: &mut D,
) -> Result<Control<D::Value, Ext>, D::Error>
where
    D: Env,
    D::Value: Clone + ProductValue,
    T: CompileTimeValue,
{
    let values: Vec<D::Value> = op
        .values
        .iter()
        .map(|ssa| domain.read_value(*ssa))
        .collect::<Result<_, _>>()?;
    let product = D::Value::new_product(values);
    Ok(Control::Return(product))
}

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Return<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        eval_return(self, domain)
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Return<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn eval(
        &self,
        domain: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        eval_return(self, domain)
    }
}

// ---------------------------------------------------------------------------
// Call
// ---------------------------------------------------------------------------

fn eval_call<D, L, T>(op: &Call<T>, domain: &mut D) -> Result<Control<D::Value, D::Ext>, D::Error>
where
    D: Env,
    D::Stages: HasStageInfo<L>,
    D::Value: Clone,
    D::Error: From<InterpreterError>,
    L: Dialect,
    T: CompileTimeValue,
{
    let args = domain.read_many(op.args())?;
    let stage_id = domain.current_stage();
    let callee = domain.resolve_function_for::<L>(op.target(), stage_id)?;
    Ok(Control::Call {
        callee,
        stage: stage_id,
        args,
        results: op.results().to_vec(),
    })
}

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Call<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        eval_call::<_, L, T>(self, domain)
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Call<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn eval(
        &self,
        domain: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        eval_call::<_, L, T>(self, domain)
    }
}

// ---------------------------------------------------------------------------
// Lifted — delegates to inner types
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Lifted<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        match self {
            Lifted::FunctionBody(op) => op.eval(domain),
            Lifted::Bind(op) => op.eval(domain),
            Lifted::Call(op) => op.eval(domain),
            Lifted::Return(op) => op.eval(domain),
        }
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Lifted<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn eval(
        &self,
        domain: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        match self {
            Lifted::FunctionBody(op) => op.eval(domain),
            Lifted::Bind(op) => op.eval(domain),
            Lifted::Call(op) => op.eval(domain),
            Lifted::Return(op) => op.eval(domain),
        }
    }
}

// ---------------------------------------------------------------------------
// Lexical — delegates to inner types
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Lexical<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        match self {
            Lexical::FunctionBody(op) => op.eval(domain),
            Lexical::Lambda(op) => op.eval(domain),
            Lexical::Call(op) => op.eval(domain),
            Lexical::Return(op) => op.eval(domain),
        }
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for Lexical<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn eval(
        &self,
        domain: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        match self {
            Lexical::FunctionBody(op) => op.eval(domain),
            Lexical::Lambda(op) => op.eval(domain),
            Lexical::Call(op) => op.eval(domain),
            Lexical::Return(op) => op.eval(domain),
        }
    }
}
