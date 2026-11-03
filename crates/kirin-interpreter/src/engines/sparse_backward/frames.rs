//! The demand frame: one work item = dispatch the rules that translate its
//! demand, in the callable context the work carries.

use kirin_ir::{CompileStage, HasBottom, Lattice, SSAValue, StageMeta, Statement};

use super::{DemandContext, SparseBackwardDriver, SparseBackwardEffect};
use crate::{
    Frame, FrameEffect, InterpDispatch, InterpreterError, Linker, SparseBackwardSemantic,
    StageQuery,
};

/// One work-item analysis: dispatch each statement whose rule can translate
/// the item's demand, under the item's callable context, accumulating the
/// local demands the rules raise and the callee parameter facts of the calls
/// they cross.
pub struct DemandFrame<K, V> {
    context: K,
    stage: CompileStage,
    work: Vec<Statement>,
    collected: Vec<(SSAValue, V)>,
}

impl<K, V> DemandFrame<K, V> {
    pub(super) fn new(context: K, stage: CompileStage, work: Vec<Statement>) -> Self {
        Self {
            context,
            stage,
            work,
            collected: Vec::new(),
        }
    }
}

// This engine has one leaf continuation and never pushes a child, so its frame
// is also its complete stack-item composition.
impl<'ir, S, V, E, Lk, P, Sem> Frame<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>>
    for DemandFrame<P::Key, V>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    type Completion = Vec<(SSAValue, V)>;

    fn step_into(
        mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        match self.work.pop() {
            Some(statement) => {
                match interp.run_statement(&self.context, self.stage, statement)? {
                    SparseBackwardEffect::Demands(demands) => self.collected.extend(demands),
                    // Summarize, don't descend: the callee context is analyzed
                    // as its own work.
                    SparseBackwardEffect::Call(call) => {
                        // Keep the facts the rule raised before returning the call.
                        self.collected.extend(interp.inner_mut().take_demands());
                        self.collected
                            .extend(interp.summarize_call(self.stage, call)?);
                    }
                }
                Ok(FrameEffect::Continue(self))
            }
            None => Ok(FrameEffect::Complete(self.collected)),
        }
    }

    fn resume_done_into(
        self,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        Err(E::from(InterpreterError::Custom(
            "demand frames never push children",
        )))
    }

    fn resume_into(
        self,
        _completion: Self::Completion,
        _interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        Err(E::from(InterpreterError::Custom(
            "demand frames never push children",
        )))
    }
}
