use std::hash::Hash;

use crate::{Frame, InterpreterError};

use super::{
    FixpointPhase, OwnerSemantics, StandardFixpointInterpreter, Summary, SummaryDependencies,
    SummaryDependency, SummaryDependencyIndex, SummaryEffect, WorkItem,
};

impl<'ir, Stage, K, F, C, E, S, Store, Deps>
    StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>
where
    K: Clone + Eq + Hash,
    S: Summary,
{
    pub fn ensure_owner<Sem>(&mut self, semantics: &mut Sem, owner: K) -> Result<(), E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<Deps::Error>,
    {
        if self.summaries.contains_key(&owner) {
            return Ok(());
        }

        let summary = semantics.bottom_summary(self, &owner)?;
        self.summaries.insert(owner.clone(), summary);
        self.deps.ensure_owner(&owner).map_err(E::from)?;
        Ok(())
    }

    pub fn solve<Sem>(&mut self, semantics: &mut Sem, entry: K) -> Result<(), E>
    where
        F: Frame<Self, F, C, E>,
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
    {
        self.ensure_owner(semantics, entry.clone())?;
        self.phase = FixpointPhase::Widen;
        self.schedule(entry);
        self.drain_worklist(semantics)
    }

    pub fn run_narrowing<Sem>(&mut self, semantics: &mut Sem, iterations: usize) -> Result<(), E>
    where
        F: Frame<Self, F, C, E>,
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
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

    pub fn drain_worklist<Sem>(&mut self, semantics: &mut Sem) -> Result<(), E>
    where
        F: Frame<Self, F, C, E>,
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
    {
        while let Some(WorkItem::Analyze(owner)) = self.worklist.pop_front() {
            self.analyze_owner(semantics, owner)?;
        }

        Ok(())
    }

    pub fn merge_summary<Sem>(
        &mut self,
        semantics: &mut Sem,
        owner: K,
        candidate: S,
    ) -> Result<bool, E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
    {
        self.ensure_owner(semantics, owner.clone())?;
        let summary = self
            .summaries
            .get_mut(&owner)
            .ok_or(InterpreterError::Custom(
                "missing summary after owner initialization",
            ))
            .map_err(E::from)?;

        let change = summary.merge(self.phase, candidate, &mut self.strategy);
        let changed = change.is_some();
        if let Some(change) = change {
            let deps = self
                .deps
                .on_summary_changed(&owner, change)
                .map_err(E::from)?;
            self.schedule_dependencies(semantics, deps)?;
        }
        Ok(changed)
    }

    fn schedule_dependencies<Sem>(
        &mut self,
        semantics: &mut Sem,
        deps: SummaryDependencies<K>,
    ) -> Result<(), E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<Deps::Error>,
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

    fn analyze_owner<Sem>(&mut self, semantics: &mut Sem, owner: K) -> Result<(), E>
    where
        F: Frame<Self, F, C, E>,
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
    {
        let summary = self
            .summaries
            .get(&owner)
            .ok_or(InterpreterError::Custom("missing summary for work item"))
            .map_err(E::from)?
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
        effect: SummaryEffect<K, S>,
    ) -> Result<(), E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
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

    fn apply_pending_summary_effects<Sem>(&mut self, semantics: &mut Sem) -> Result<(), E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        Deps: SummaryDependencyIndex<K>,
        E: From<InterpreterError> + From<Deps::Error>,
    {
        let effects = std::mem::take(&mut self.pending_effects);
        for effect in effects {
            self.apply_summary_effect(semantics, effect)?;
        }
        Ok(())
    }
}
