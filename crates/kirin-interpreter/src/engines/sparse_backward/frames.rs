//! The demand frame: one work item = dispatch one statement's rule in the
//! callable context the work carries.

use kirin_ir::{CompileStage, HasBottom, Lattice, SSAValue, StageMeta, Statement};

use super::{DemandContext, SparseBackwardDriver, SparseBackwardEffect, SparseBackwardTransfer};
use crate::{
    EnvIndex, Frame, FrameEffect, InterpDispatch, InterpreterError, Linker, SparseBackwardSemantic,
    StageQuery,
};

/// One work-item analysis: dispatch the statement's rule in the item's
/// context, collecting the local demands it raises and, for a call, the
/// callee's parameter facts on the call's arguments.
pub struct DemandFrame<V> {
    env: EnvIndex,
    stage: CompileStage,
    statement: Option<Statement>,
    collected: Vec<(SSAValue, V)>,
}

impl<V> DemandFrame<V> {
    pub(super) fn new(env: EnvIndex, stage: CompileStage, statement: Statement) -> Self {
        Self {
            env,
            stage,
            statement: Some(statement),
            collected: Vec::new(),
        }
    }
}

// This engine has one leaf continuation and never pushes a child, so its frame
// is also its complete stack-item composition.
impl<'ir, S, V, E, Lk, P, Sem> Frame<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>>
    for DemandFrame<V>
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>,
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
        let Some(statement) = self.statement.take() else {
            return Ok(FrameEffect::Complete(self.collected));
        };
        match interp
            .inner_mut()
            .run_statement(self.env, self.stage, statement)?
        {
            SparseBackwardEffect::Demands(demands) => self.collected.extend(demands),
            // Summarize, don't descend: the callee context is analyzed as its
            // own work.
            SparseBackwardEffect::Call(call) => {
                // Keep the facts the rule raised before returning the call.
                self.collected.extend(interp.inner_mut().take_demands());
                self.collected
                    .extend(interp.summarize_call(self.stage, call)?);
            }
        }
        Ok(FrameEffect::Continue(self))
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
