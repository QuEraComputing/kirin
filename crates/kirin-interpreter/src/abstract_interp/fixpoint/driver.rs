use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;

use kirin_ir::Pipeline;
use smallvec::SmallVec;

use crate::FixpointProfile;

use super::{FixpointPhase, OwnerSummaryDeps, Summary, SummaryEffect, WorkItem};

pub struct StandardFixpointInterpreter<'ir, P: FixpointProfile, Store, Deps> {
    pub(super) pipeline: &'ir Pipeline<P::Stage>,
    pub(super) summaries: HashMap<P::SummaryKey, P::Summary>,
    pub(super) store: Store,
    pub(super) deps: Deps,
    pub(super) worklist: VecDeque<WorkItem<P::SummaryKey>>,
    pub(super) frame_stack: SmallVec<[P::Frame; 8]>,
    pub(super) current_owner: Option<P::SummaryKey>,
    pub(super) pending_effects: Vec<SummaryEffect<P::SummaryKey, P::Summary>>,
    pub(super) phase: FixpointPhase,
    pub(super) strategy: <P::Summary as Summary>::Strategy,
    _marker: PhantomData<P>,
}

pub type SimpleFixpointInterpreter<'ir, P, Store> = StandardFixpointInterpreter<
    'ir,
    P,
    Store,
    OwnerSummaryDeps<<P as FixpointProfile>::SummaryKey>,
>;

impl<'ir, P: FixpointProfile, Store, Deps> StandardFixpointInterpreter<'ir, P, Store, Deps> {
    pub fn with_dependency_index(
        pipeline: &'ir Pipeline<P::Stage>,
        store: Store,
        strategy: <P::Summary as Summary>::Strategy,
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

    pub fn pipeline(&self) -> &'ir Pipeline<P::Stage> {
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

    pub fn summary(&self, owner: &P::SummaryKey) -> Option<&P::Summary> {
        self.summaries.get(owner)
    }

    pub fn summaries(&self) -> &HashMap<P::SummaryKey, P::Summary> {
        &self.summaries
    }

    pub fn schedule(&mut self, owner: P::SummaryKey) {
        self.worklist.push_back(WorkItem::Analyze(owner));
    }

    pub fn frame_stack(&self) -> &[P::Frame] {
        self.frame_stack.as_slice()
    }

    pub fn clear_frame_stack(&mut self) {
        self.frame_stack.clear();
    }

    pub fn current_owner(&self) -> Option<&P::SummaryKey> {
        self.current_owner.as_ref()
    }

    pub fn push_summary_effect(&mut self, effect: SummaryEffect<P::SummaryKey, P::Summary>) {
        self.pending_effects.push(effect);
    }
}

impl<'ir, P: FixpointProfile, Store>
    StandardFixpointInterpreter<'ir, P, Store, OwnerSummaryDeps<P::SummaryKey>>
{
    pub fn new(
        pipeline: &'ir Pipeline<P::Stage>,
        store: Store,
        strategy: <P::Summary as Summary>::Strategy,
    ) -> Self {
        Self::with_dependency_index(pipeline, store, strategy, OwnerSummaryDeps::new())
    }
}
