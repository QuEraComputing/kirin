use kirin_ir::{Dialect, HasStageInfo, SpecializedFunction, StageMeta, SupportsStageDispatch};

use super::{
    AbstractInterpreter, SummaryCache, fixpoint::AnalyzeDynAction, interp::SummaryInserter,
};
use crate::{
    AbstractValue, CallSemantics, Interpretable, InterpreterError, Staged, result::AnalysisResult,
    stage::expect_stage_id,
};

impl<'a, 'ir, V, S, E, G, L> Staged<'a, 'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>>
        + CallSemantics<'ir, AbstractInterpreter<'ir, V, S, E, G>, Result = AnalysisResult<V>>
        + 'ir,
    for<'x> S: SupportsStageDispatch<AnalyzeDynAction<'x, 'ir, V, S, E, G>, AnalysisResult<V>, E>,
{
    /// Analyze a specialized function.
    pub fn analyze(self, callee: SpecializedFunction, args: &[V]) -> Result<AnalysisResult<V>, E> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.call_handler = Some(AbstractInterpreter::analyze);
        self.interp
            .analyze_with_stage_id::<L>(callee, stage_id, args)
    }
}

impl<'a, 'ir, V, S, E, G, L> Staged<'a, 'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
where
    V: AbstractValue + Clone + 'ir,
    S: StageMeta + 'ir,
    L: Dialect,
{
    /// Look up the best cached summary for `callee` in this stage.
    pub fn summary(&self, callee: SpecializedFunction, args: &[V]) -> Option<&AnalysisResult<V>> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.summary_in_stage(stage_id, callee, args)
    }

    /// Look up the full summary cache for `callee` in this stage.
    pub fn summary_cache(&self, callee: SpecializedFunction) -> Option<&SummaryCache<V>> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.summary_cache_in_stage(stage_id, callee)
    }

    /// Return a builder for inserting a function summary in this stage.
    pub fn insert_summary(
        self,
        callee: SpecializedFunction,
    ) -> SummaryInserter<'a, 'ir, V, S, E, G> {
        let stage_id = expect_stage_id(self.stage);
        self.interp.insert_summary_in_stage(stage_id, callee)
    }

    /// Mark all computed entries for `callee` in this stage as invalidated.
    pub fn invalidate_summary(&mut self, callee: SpecializedFunction) -> usize {
        let stage_id = expect_stage_id(self.stage);
        self.interp.invalidate_summary_in_stage(stage_id, callee)
    }

    /// Unconditionally remove all summaries for `callee` in this stage.
    pub fn remove_summary(&mut self, callee: SpecializedFunction) -> bool {
        let stage_id = expect_stage_id(self.stage);
        self.interp.remove_summary_in_stage(stage_id, callee)
    }
}
