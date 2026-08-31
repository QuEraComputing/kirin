use kirin_ir::{Dialect, StageInfo, StageMeta, Statement};

use crate::{CallableBody, Interp, InterpreterError};

/// Statement semantics. The single trait dialect authors implement.
///
/// The engine type `I` is the object a rule receives directly; the `Semantics`
/// parameter is a compile-time [`SemanticKey`](crate::SemanticKey) (e.g.
/// [`ForwardEval`](crate::ForwardEval)) naming *which* interpretation this
/// impl describes — never a raw solver shape. The same dialect type carries
/// one impl per key (a forward-evaluation rule, a demand rule, a liveness
/// rule, downstream keys, …) without coherence conflicts, and two keys may
/// share one [`AnalysisShape`](crate::AnalysisShape).
pub trait Interpretable<I: Interp, Semantics>: Dialect {
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error>;
}

/// Function-entry semantics for callable statements.
///
/// Implemented by statements that define function bodies (e.g.
/// `kirin_function::Function`); describes the [`CallableBody`] an engine enters
/// when the function is invoked. Derived on language enums with
/// `#[derive(FunctionEntry)]` where `#[callable]` marks the variants that wrap
/// callable statements.
pub trait FunctionEntry: Dialect {
    /// Return this definition's callable body, or `None` when the statement is
    /// not callable.
    ///
    /// Body discovery is structural and therefore independent of an engine's
    /// value domain and boundary inputs.
    fn function_entry(&self) -> Option<CallableBody>;
}

/// Monomorphic statement dispatch over a stage enum.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution and function entry through this trait; compiler
/// authors derive it and never call it.
///
/// Keyed on the engine `I` alone: the semantic key dispatched is always
/// `I::Semantics`, so a `StrongDemand` dispatch can never be paired with a
/// `ClassicLiveness` engine — the engine's own key is the single source of
/// which rules run.
pub trait InterpDispatch<I: Interp>: StageMeta {
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error>;

    fn dispatch_function_entry(&self, definition: Statement) -> Result<CallableBody, I::Error>;
}

impl<I, L> InterpDispatch<I> for StageInfo<L>
where
    I: Interp,
    L: Dialect + Interpretable<I, <I as Interp>::Semantics> + FunctionEntry,
{
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(interp)
    }

    fn dispatch_function_entry(&self, definition: Statement) -> Result<CallableBody, I::Error> {
        let callable = definition.definition(self).clone();
        callable
            .function_entry()
            .ok_or_else(|| I::Error::from(InterpreterError::NotCallable(definition)))
    }
}
