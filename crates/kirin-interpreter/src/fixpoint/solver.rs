//! The worklist loop.
//!
//! [`drain_worklist`](StandardFixpointInterpreter::drain_worklist) pops work
//! items and analyses each one: its entry frame runs to completion (via
//! [`run_frame`](StandardFixpointInterpreter::run_frame), see `runner.rs`), and
//! the [`WorkAnalysis`] handles the completion, scheduling whatever must rerun.
//! This nests *outside* the frame loop: the frame stack is the transfer, this
//! worklist is the fixpoint.

use crate::{Frame, Interp};

use super::{FixpointProfile, StandardFixpointInterpreter, WorkAnalysis};

impl<I, P, Store> StandardFixpointInterpreter<I, P, Store>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Schedule several work items and analyse them (and everything they
    /// schedule) to a fixpoint.
    ///
    /// A dataflow equation system with no single entry — e.g. backward liveness,
    /// where every block must be visited even if unreachable from one seed —
    /// schedules every work item up front rather than relying on dependencies to
    /// reach them.
    pub fn solve_many<Analysis>(
        &mut self,
        analysis: &mut Analysis,
        entries: impl IntoIterator<Item = P::Work>,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Analysis: WorkAnalysis<Self, P::Work, P::Frame, P::Completion, I::Error>,
    {
        for entry in entries {
            self.schedule(entry);
        }
        self.drain_worklist(analysis)
    }

    /// Pop and analyse work items until the worklist is empty.
    pub fn drain_worklist<Analysis>(&mut self, analysis: &mut Analysis) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Analysis: WorkAnalysis<Self, P::Work, P::Frame, P::Completion, I::Error>,
    {
        while let Some(work) = self.worklist.pop_front() {
            self.analyze_work(analysis, work)?;
        }

        Ok(())
    }

    fn analyze_work<Analysis>(
        &mut self,
        analysis: &mut Analysis,
        work: P::Work,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Analysis: WorkAnalysis<Self, P::Work, P::Frame, P::Completion, I::Error>,
    {
        self.current_work = Some(work.clone());
        let root = match analysis.entry_frame(self, &work) {
            Ok(root) => root,
            Err(error) => {
                self.current_work = None;
                return Err(error);
            }
        };
        let completion = match self.run_frame(root) {
            Ok(completion) => completion,
            Err(error) => {
                self.current_work = None;
                return Err(error);
            }
        };
        self.current_work = None;
        analysis.complete_work(self, work, completion)
    }
}
