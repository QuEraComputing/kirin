use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::Pipeline;
use smallvec::SmallVec;

use super::{FixpointPhase, OwnerSummaryDeps, Summary, SummaryEffect, WorkItem};

pub struct StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>
where
    S: Summary,
{
    pub(super) pipeline: &'ir Pipeline<Stage>,
    pub(super) summaries: HashMap<K, S>,
    pub(super) store: Store,
    pub(super) deps: Deps,
    pub(super) worklist: VecDeque<WorkItem<K>>,
    pub(super) frame_stack: SmallVec<[F; 8]>,
    pub(super) current_owner: Option<K>,
    pub(super) pending_effects: Vec<SummaryEffect<K, S>>,
    pub(super) phase: FixpointPhase,
    pub(super) strategy: S::Strategy,
    _marker: PhantomData<(F, C, E)>,
}

pub type SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store> =
    StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, OwnerSummaryDeps<K>>;

impl<'ir, Stage, K, F, C, E, S, Store, Deps>
    StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>
where
    K: Clone + Eq + Hash,
    S: Summary,
{
    pub fn with_dependency_index(
        pipeline: &'ir Pipeline<Stage>,
        store: Store,
        strategy: S::Strategy,
        deps: Deps,
    ) -> Self {
        Self {
            pipeline,
            summaries: HashMap::new(),
            store,
            deps,
            worklist: VecDeque::new(),
            frame_stack: SmallVec::new(),
            current_owner: None,
            pending_effects: Vec::new(),
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

    pub fn dependency_index(&self) -> &Deps {
        &self.deps
    }

    pub fn dependency_index_mut(&mut self) -> &mut Deps {
        &mut self.deps
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

    pub fn frame_stack(&self) -> &[F] {
        self.frame_stack.as_slice()
    }

    pub fn clear_frame_stack(&mut self) {
        self.frame_stack.clear();
    }

    pub fn current_owner(&self) -> Option<&K> {
        self.current_owner.as_ref()
    }

    pub fn push_summary_effect(&mut self, effect: SummaryEffect<K, S>) {
        self.pending_effects.push(effect);
    }
}

impl<'ir, Stage, K, F, C, E, S, Store>
    StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, OwnerSummaryDeps<K>>
where
    K: Clone + Eq + Hash,
    S: Summary,
{
    pub fn new(pipeline: &'ir Pipeline<Stage>, store: Store, strategy: S::Strategy) -> Self {
        Self::with_dependency_index(pipeline, store, strategy, OwnerSummaryDeps::new())
    }
}
