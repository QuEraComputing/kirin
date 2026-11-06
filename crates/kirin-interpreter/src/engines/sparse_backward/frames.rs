//! The demand frame: one work item = dispatch one statement's rule in the
//! callable context the work carries.

use kirin_ir::{CompileStage, HasBottom, Lattice, StageMeta, Statement};

use super::{DemandContext, SparseBackwardDriver, SparseBackwardEffect, SparseBackwardTransfer};
use crate::{
    EnvIndex, Frame, FrameEffect, InterpDispatch, InterpreterError, Linker, SparseBackwardSemantic,
    StageQuery,
};

/// One work-item analysis: dispatch the statement's rule in the item's
/// context and, for a call, summarize the call.
pub struct DemandFrame {
    env: EnvIndex,
    stage: CompileStage,
    statement: Option<Statement>,
}

impl DemandFrame {
    pub(super) fn new(env: EnvIndex, stage: CompileStage, statement: Statement) -> Self {
        Self {
            env,
            stage,
            statement: Some(statement),
        }
    }
}

// This engine has one leaf continuation and never pushes a child, so its frame
// is also its complete stack-item composition.
impl<'ir, S, V, E, Lk, P, Sem> Frame<SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>> for DemandFrame
where
    S: StageMeta + StageQuery + InterpDispatch<SparseBackwardTransfer<'ir, S, V, E, Lk, P, Sem>>,
    V: Clone + PartialEq + Lattice + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: DemandContext<V>,
    Sem: SparseBackwardSemantic,
{
    type Completion = ();

    fn step_into(
        mut self,
        interp: &mut SparseBackwardDriver<'ir, S, V, E, Lk, P, Sem>,
    ) -> Result<FrameEffect<Self, Self::Completion>, E> {
        let Some(statement) = self.statement.take() else {
            return Ok(FrameEffect::Complete(()));
        };
        // Summarize a call, don't descend: the callee context is analyzed as
        // its own work.
        if let SparseBackwardEffect::Call(call) = interp
            .inner_mut()
            .run_statement(self.env, self.stage, statement)?
        {
            interp.summarize_call(self.stage, call)?;
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
