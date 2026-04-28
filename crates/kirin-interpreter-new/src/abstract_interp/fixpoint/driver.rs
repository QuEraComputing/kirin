use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::Pipeline;

use super::{FixpointPhase, OwnerSemantics, Summary, SummaryEffect, WorkItem};
use crate::{Frame, InterpreterError};

pub struct SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    S: Summary,
{
    pub(super) pipeline: &'ir Pipeline<Stage>,
    summaries: HashMap<K, S>,
    pub(super) store: Store,
    worklist: VecDeque<WorkItem<K>>,
    phase: FixpointPhase,
    strategy: S::Strategy,
    _marker: PhantomData<(F, C, E)>,
}

impl<'ir, Stage, K, F, C, E, S, Store> SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    S: Summary,
{
    pub fn new(pipeline: &'ir Pipeline<Stage>, store: Store, strategy: S::Strategy) -> Self {
        Self {
            pipeline,
            summaries: HashMap::new(),
            store,
            worklist: VecDeque::new(),
            phase: FixpointPhase::Join,
            strategy,
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<Stage> {
        self.pipeline
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    pub fn phase(&self) -> FixpointPhase {
        self.phase
    }

    pub fn set_phase(&mut self, phase: FixpointPhase) {
        self.phase = phase;
    }

    pub fn summary(&self, owner: &K) -> Option<&S> {
        self.summaries.get(owner)
    }

    pub fn summaries(&self) -> &HashMap<K, S> {
        &self.summaries
    }

    pub fn schedule(&mut self, owner: K) {
        self.worklist.push_back(WorkItem::Analyze(owner));
    }

    pub fn ensure_owner<Sem>(&mut self, semantics: &mut Sem, owner: K) -> Result<(), E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
    {
        if self.summaries.contains_key(&owner) {
            return Ok(());
        }

        let summary = semantics.bottom_summary(self, &owner)?;
        self.summaries.insert(owner, summary);
        Ok(())
    }

    pub fn solve<Sem>(&mut self, semantics: &mut Sem, entry: K) -> Result<(), E>
    where
        F: Frame<Self, F, C, E>,
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        E: From<InterpreterError>,
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
        E: From<InterpreterError>,
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
        E: From<InterpreterError>,
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
        E: From<InterpreterError>,
    {
        self.ensure_owner(semantics, owner.clone())?;
        let summary = self
            .summaries
            .get_mut(&owner)
            .ok_or(InterpreterError::Custom(
                "missing summary after owner initialization",
            ))?;

        let changed = summary
            .merge(self.phase, candidate, &mut self.strategy)
            .is_some();
        if changed {
            self.schedule(owner);
        }
        Ok(changed)
    }

    fn analyze_owner<Sem>(&mut self, semantics: &mut Sem, owner: K) -> Result<(), E>
    where
        F: Frame<Self, F, C, E>,
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        E: From<InterpreterError>,
    {
        let summary = self
            .summaries
            .get(&owner)
            .ok_or(InterpreterError::Custom("missing summary for work item"))?
            .clone();
        let root = semantics.entry_frame(self, &owner, &summary)?;
        let completion = self.run_frame(root)?;
        let effect = semantics.complete_owner(self, owner, completion)?;
        self.apply_summary_effect(semantics, effect)?;
        Ok(())
    }

    fn apply_summary_effect<Sem>(
        &mut self,
        semantics: &mut Sem,
        effect: SummaryEffect<K, S>,
    ) -> Result<(), E>
    where
        Sem: OwnerSemantics<Self, K, S, F, C, E>,
        E: From<InterpreterError>,
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
}
