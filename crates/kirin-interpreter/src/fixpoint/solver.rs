//! The inter-owner convergence loop.
//!
//! [`solve_many`](StandardFixpointInterpreter::solve_many) drains an owner worklist: each
//! item analyses one owner (via [`run_frame`](StandardFixpointInterpreter::run_frame),
//! see `runner.rs`), merges the resulting summary, and — if it changed — schedules
//! the owners the dependency index says to reanalyse. This nests *outside* the
//! intra-owner frame loop: the frame stack is the transfer, this worklist is the
//! fixpoint.

use crate::{Frame, Interp, InterpreterError};

use super::{
    FixpointProfile, OwnerAnalysis, StandardFixpointInterpreter, Summary, SummaryDependencies,
    SummaryDependency, SummaryDependencyIndex, SummaryEffect, WorkItem,
};

impl<I, P, Store, Deps> StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Ensure `owner` has a (bottom) summary and is registered in the index.
    pub fn ensure_owner<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
        owner: P::SummaryKey,
    ) -> Result<(), I::Error>
    where
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        if self.summaries.contains_key(&owner) {
            return Ok(());
        }

        let summary = owner_analysis.bottom_summary(self, &owner)?;
        self.summaries.insert(owner.clone(), summary);
        self.deps
            .ensure_owner(&owner)
            .map_err(|error| I::Error::from(InterpreterError::from(error)))?;
        Ok(())
    }

    /// Seed several owners and analyse them (and everything they schedule) to a
    /// fixpoint.
    ///
    /// A dataflow equation system with no single entry — e.g. backward liveness,
    /// where every block must be visited even if unreachable from one seed — seeds
    /// every owner up front rather than relying on the dependency graph to reach
    /// them.
    pub fn solve_many<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
        entries: impl IntoIterator<Item = P::SummaryKey>,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        for entry in entries {
            self.ensure_owner(owner_analysis, entry.clone())?;
            self.schedule(entry);
        }
        self.drain_worklist(owner_analysis)
    }

    /// Pop and analyse owners until the worklist is empty.
    pub fn drain_worklist<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        while let Some(WorkItem::Analyze(owner)) = self.worklist.pop_front() {
            self.analyze_owner(owner_analysis, owner)?;
        }

        Ok(())
    }

    /// Merge `candidate` into `owner`'s summary; on change, schedule dependents.
    /// Returns whether the summary changed.
    pub fn merge_summary<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
        owner: P::SummaryKey,
        candidate: P::Summary,
    ) -> Result<bool, I::Error>
    where
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        self.ensure_owner(owner_analysis, owner.clone())?;
        let summary = self.summaries.get_mut(&owner).ok_or_else(|| {
            <I::Error as From<InterpreterError>>::from(InterpreterError::Custom(
                "missing summary after owner initialization",
            ))
        })?;

        let changed = summary.merge(candidate);
        if changed {
            let deps = self
                .deps
                .on_summary_changed(&owner)
                .map_err(|error| I::Error::from(InterpreterError::from(error)))?;
            self.schedule_dependencies(owner_analysis, deps)?;
        }
        Ok(changed)
    }

    fn schedule_dependencies<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
        deps: SummaryDependencies<P::SummaryKey>,
    ) -> Result<(), I::Error>
    where
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        for dep in deps {
            match dep {
                SummaryDependency::Reanalyze(owner) => {
                    self.ensure_owner(owner_analysis, owner.clone())?;
                    self.schedule(owner);
                }
            }
        }
        Ok(())
    }

    fn analyze_owner<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
        owner: P::SummaryKey,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        let summary = self
            .summaries
            .get(&owner)
            .ok_or_else(|| {
                <I::Error as From<InterpreterError>>::from(InterpreterError::Custom(
                    "missing summary for work item",
                ))
            })?
            .clone();
        self.current_owner = Some(owner.clone());
        let root = match owner_analysis.entry_frame(self, &owner, &summary) {
            Ok(root) => root,
            Err(error) => {
                self.current_owner = None;
                return Err(error);
            }
        };
        let completion = match self.run_frame(root) {
            Ok(completion) => completion,
            Err(error) => {
                self.current_owner = None;
                return Err(error);
            }
        };
        self.current_owner = None;
        let effect = owner_analysis.complete_owner(self, owner, completion)?;
        self.apply_summary_effect(owner_analysis, effect)?;
        Ok(())
    }

    fn apply_summary_effect<Analysis>(
        &mut self,
        owner_analysis: &mut Analysis,
        effect: SummaryEffect<P::SummaryKey, P::Summary>,
    ) -> Result<(), I::Error>
    where
        Analysis: OwnerAnalysis<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        match effect {
            SummaryEffect::None => Ok(()),
            SummaryEffect::Update { owner, candidate } => {
                self.merge_summary(owner_analysis, owner, candidate)?;
                Ok(())
            }
            SummaryEffect::Many(updates) => {
                for (owner, candidate) in updates {
                    self.merge_summary(owner_analysis, owner, candidate)?;
                }
                Ok(())
            }
        }
    }
}
