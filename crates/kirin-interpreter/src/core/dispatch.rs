use kirin_ir::{Dialect, StageInfo, StageMeta, Statement};

use crate::Interp;

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

/// Monomorphic statement dispatch over a stage enum.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution through this trait; compiler
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
}

impl<I, L> InterpDispatch<I> for StageInfo<L>
where
    I: Interp,
    L: Dialect + Interpretable<I, <I as Interp>::Semantics>,
{
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(interp)
    }
}
