//! The inter-owner convergence loop.
//!
//! [`solve`](StandardFixpointInterpreter::solve) drains an owner worklist: each
//! item analyses one owner (via [`run_frame`](StandardFixpointInterpreter::run_frame),
//! see `runner.rs`), merges the resulting summary, and — if it changed — schedules
//! the owners the dependency index says to reanalyse. This nests *outside* the
//! intra-owner frame loop: the frame stack is the transfer, this worklist is the
//! fixpoint.

use crate::{Frame, Interp, InterpreterError};

use super::{
    FixpointPhase, FixpointProfile, OwnerSemantics, StandardFixpointInterpreter, Summary,
    SummaryDependencies, SummaryDependency, SummaryDependencyIndex, SummaryEffect, WorkItem,
};

impl<I, P, Store, Deps> StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Ensure `owner` has a (bottom) summary and is registered in the index.
    pub fn ensure_owner<Sem>(
        &mut self,
        semantics: &mut Sem,
        owner: P::SummaryKey,
    ) -> Result<(), I::Error>
    where
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        if self.summaries.contains_key(&owner) {
            return Ok(());
        }

        let summary = semantics.bottom_summary(self, &owner)?;
        self.summaries.insert(owner.clone(), summary);
        self.deps
            .ensure_owner(&owner)
            .map_err(|error| I::Error::from(InterpreterError::from(error)))?;
        Ok(())
    }

    /// Analyse `entry` and everything it transitively schedules, to a fixpoint.
    pub fn solve<Sem>(&mut self, semantics: &mut Sem, entry: P::SummaryKey) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        self.ensure_owner(semantics, entry.clone())?;
        self.phase = FixpointPhase::Widen;
        self.schedule(entry);
        self.drain_worklist(semantics)
    }

    /// Seed several owners and analyse them (and everything they schedule) to a
    /// fixpoint.
    ///
    /// A dataflow equation system with no single entry — e.g. backward liveness,
    /// where every block must be visited even if unreachable from one seed — seeds
    /// every owner up front rather than relying on the dependency graph to reach
    /// them.
    pub fn solve_many<Sem>(
        &mut self,
        semantics: &mut Sem,
        entries: impl IntoIterator<Item = P::SummaryKey>,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        self.phase = FixpointPhase::Widen;
        for entry in entries {
            self.ensure_owner(semantics, entry.clone())?;
            self.schedule(entry);
        }
        self.drain_worklist(semantics)
    }

    /// Re-run every owner under [`FixpointPhase::Narrow`] up to `iterations`
    /// times, refining a post-fixpoint back towards the least fixpoint.
    pub fn run_narrowing<Sem>(
        &mut self,
        semantics: &mut Sem,
        iterations: usize,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        self.phase = FixpointPhase::Narrow;
        for owner in self.summaries.keys().cloned().collect::<Vec<_>>() {
            self.schedule(owner);
        }

        for _ in 0..iterations {
            if self.worklist.is_empty() {
                break;
            }
            self.drain_worklist(semantics)?;
        }

        Ok(())
    }

    /// Pop and analyse owners until the worklist is empty.
    pub fn drain_worklist<Sem>(&mut self, semantics: &mut Sem) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        while let Some(WorkItem::Analyze(owner)) = self.worklist.pop_front() {
            self.analyze_owner(semantics, owner)?;
        }

        Ok(())
    }

    /// Merge `candidate` into `owner`'s summary; on change, schedule dependents.
    /// Returns whether the summary changed.
    pub fn merge_summary<Sem>(
        &mut self,
        semantics: &mut Sem,
        owner: P::SummaryKey,
        candidate: P::Summary,
    ) -> Result<bool, I::Error>
    where
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        self.ensure_owner(semantics, owner.clone())?;
        let summary = self.summaries.get_mut(&owner).ok_or_else(|| {
            <I::Error as From<InterpreterError>>::from(InterpreterError::Custom(
                "missing summary after owner initialization",
            ))
        })?;

        let change = summary.merge(self.phase, candidate, &mut self.strategy);
        let changed = change.is_some();
        if let Some(change) = change {
            let deps = self
                .deps
                .on_summary_changed(&owner, change)
                .map_err(|error| I::Error::from(InterpreterError::from(error)))?;
            self.schedule_dependencies(semantics, deps)?;
        }
        Ok(changed)
    }

    fn schedule_dependencies<Sem>(
        &mut self,
        semantics: &mut Sem,
        deps: SummaryDependencies<P::SummaryKey>,
    ) -> Result<(), I::Error>
    where
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        for dep in deps {
            match dep {
                SummaryDependency::Reanalyze(owner) => {
                    self.ensure_owner(semantics, owner.clone())?;
                    self.schedule(owner);
                }
            }
        }
        Ok(())
    }

    fn analyze_owner<Sem>(
        &mut self,
        semantics: &mut Sem,
        owner: P::SummaryKey,
    ) -> Result<(), I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
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
        let root = match semantics.entry_frame(self, &owner, &summary) {
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
        let effect = semantics.complete_owner(self, owner, completion)?;
        self.apply_summary_effect(semantics, effect)?;
        self.apply_pending_summary_effects(semantics)?;
        Ok(())
    }

    fn apply_summary_effect<Sem>(
        &mut self,
        semantics: &mut Sem,
        effect: SummaryEffect<P::SummaryKey, P::Summary>,
    ) -> Result<(), I::Error>
    where
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        match effect {
            SummaryEffect::None => Ok(()),
            SummaryEffect::Update { owner, candidate } => {
                self.merge_summary(semantics, owner, candidate)?;
                Ok(())
            }
            SummaryEffect::Many(updates) => {
                for (owner, candidate) in updates {
                    self.merge_summary(semantics, owner, candidate)?;
                }
                Ok(())
            }
        }
    }

    fn apply_pending_summary_effects<Sem>(&mut self, semantics: &mut Sem) -> Result<(), I::Error>
    where
        Sem: OwnerSemantics<Self, P::SummaryKey, P::Summary, P::Frame, P::Completion, I::Error>,
        Deps: SummaryDependencyIndex<P::SummaryKey>,
        InterpreterError: From<Deps::Error>,
    {
        let effects = std::mem::take(&mut self.pending_effects);
        for effect in effects {
            self.apply_summary_effect(semantics, effect)?;
        }
        Ok(())
    }
}
