use std::marker::PhantomData;

use kirin_ir::{
    Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta, SupportsStageDispatch,
};

use super::{AbstractInterpreter, fixpoint::AnalyzeDynAction};
use crate::{
    AbstractValue, EvalCall, Interpretable, Interpreter, InterpreterError, result::AnalysisResult,
};

/// Typed-stage API builder resolved from the interpreter's active stage.
pub struct InStage<'a, 'ir, V, S, E, G, L>
where
    S: StageMeta,
{
    interp: &'a mut AbstractInterpreter<'ir, V, S, E, G>,
    marker: PhantomData<L>,
}

/// API builder for an explicitly resolved [`StageInfo`].
pub struct WithStage<'a, 'ir, V, S, E, G, L>
where
    S: StageMeta,
    L: Dialect,
{
    interp: &'a mut AbstractInterpreter<'ir, V, S, E, G>,
    stage: &'ir StageInfo<L>,
}

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: StageMeta + 'ir,
{
    /// Resolve typed-stage APIs from the current active stage.
    pub fn in_stage<L>(&mut self) -> InStage<'_, 'ir, V, S, E, G, L> {
        InStage {
            interp: self,
            marker: PhantomData,
        }
    }

    /// Bind APIs to an explicit stage reference.
    pub fn with_stage<L>(&mut self, stage: &'ir StageInfo<L>) -> WithStage<'_, 'ir, V, S, E, G, L>
    where
        L: Dialect,
    {
        WithStage {
            interp: self,
            stage,
        }
    }
}

impl<'a, 'ir, V, S, E, G, L> InStage<'a, 'ir, V, S, E, G, L>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
        + EvalCall<'ir, AbstractInterpreter<'ir, V, S, E, G>, L, Result = AnalysisResult<V>>
        + 'ir,
    for<'x> S: SupportsStageDispatch<AnalyzeDynAction<'x, 'ir, V, S, E, G>, AnalysisResult<V>, E>,
{
    fn resolve_active_stage_info(&self) -> Result<&'ir StageInfo<L>, E> {
        let stage_id = self.interp.active_stage();
        self.interp.resolve_stage_info::<L>(stage_id)
    }

    /// Analyze a specialized function using strict typed-stage resolution.
    pub fn analyze(self, callee: SpecializedFunction, args: &[V]) -> Result<AnalysisResult<V>, E> {
        let stage = self.resolve_active_stage_info()?;
        self.interp.with_stage(stage).analyze(callee, args)
    }
}

impl<'a, 'ir, V, S, E, G, L> WithStage<'a, 'ir, V, S, E, G, L>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
        + EvalCall<'ir, AbstractInterpreter<'ir, V, S, E, G>, L, Result = AnalysisResult<V>>
        + 'ir,
    for<'x> S: SupportsStageDispatch<AnalyzeDynAction<'x, 'ir, V, S, E, G>, AnalysisResult<V>, E>,
{
    /// Analyze a specialized function in an explicit stage.
    pub fn analyze(self, callee: SpecializedFunction, args: &[V]) -> Result<AnalysisResult<V>, E> {
        let stage_id = self
            .stage
            .stage_id()
            .expect("stage info must be attached to a pipeline stage");
        self.interp.call_handler = Some(AbstractInterpreter::analyze);
        self.interp
            .analyze_with_stage_id::<L>(callee, stage_id, args)
    }
}
