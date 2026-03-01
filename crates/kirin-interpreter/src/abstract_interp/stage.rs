use kirin_ir::{Dialect, HasStageInfo, SpecializedFunction, StageMeta, SupportsStageDispatch};

use super::{AbstractInterpreter, fixpoint::AnalyzeDynAction};
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
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
        + CallSemantics<'ir, AbstractInterpreter<'ir, V, S, E, G>, L, Result = AnalysisResult<V>>
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
