use kirin_ir::{Dialect, HasStageInfo, SpecializedFunction, StageMeta, SupportsStageDispatch};

use super::{AbstractInterpreter, fixpoint::AnalyzeDynAction};
use crate::{
    AbstractValue, EvalCall, InStage, Interpretable, Interpreter, InterpreterError, WithStage,
    result::AnalysisResult, stage::expect_stage_id,
};

impl<'a, 'ir, V, S, E, G, L> InStage<'a, AbstractInterpreter<'ir, V, S, E, G>, L>
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
    /// Analyze a specialized function using strict typed-stage resolution.
    pub fn analyze(self, callee: SpecializedFunction, args: &[V]) -> Result<AnalysisResult<V>, E> {
        let stage = self.resolve_active_stage_info()?;
        self.interp.with_stage(stage).analyze(callee, args)
    }
}

impl<'a, 'ir, V, S, E, G, L> WithStage<'a, 'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
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
        let stage_id = expect_stage_id(self.stage);
        self.interp.call_handler = Some(AbstractInterpreter::analyze);
        self.interp
            .analyze_with_stage_id::<L>(callee, stage_id, args)
    }
}
