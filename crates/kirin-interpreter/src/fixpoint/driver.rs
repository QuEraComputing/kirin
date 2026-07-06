//! The owner-summary fixpoint driver.
//!
//! [`StandardFixpointInterpreter`] wraps one [`Interp`] (`inner`) and adds the
//! outer, inter-owner convergence loop: a per-owner summary map, an owner
//! worklist, a pluggable dependency index, and the widen/narrow phase state. It
//! *is* an [`Interp`] itself (by delegation to `inner`, see `delegates.rs`), so
//! frames and dialect rules see a single interpreter. The intra-owner traversal
//! runs on the frame stack via [`run_frame`](Self::run_frame); the inter-owner
//! iteration runs via [`solve`](Self::solve) (see `solver.rs`).

use std::collections::{HashMap, VecDeque};

use crate::Interp;

use super::{FixpointPhase, FixpointProfile, OwnerSummaryDeps, Summary, SummaryEffect, WorkItem};

/// Drives a [`FixpointProfile`] over an interpreter `I` to an owner-summary
/// fixpoint.
pub struct StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    pub(super) inner: I,
    pub(super) summaries: HashMap<P::SummaryKey, P::Summary>,
    pub(super) store: Store,
    pub(super) deps: Deps,
    pub(super) worklist: VecDeque<WorkItem<P::SummaryKey>>,
    pub(super) frame_stack: Vec<P::Frame>,
    pub(super) current_owner: Option<P::SummaryKey>,
    pub(super) pending_effects: Vec<SummaryEffect<P::SummaryKey, P::Summary>>,
    pub(super) phase: FixpointPhase,
    pub(super) strategy: <P::Summary as Summary>::Strategy,
}

/// A [`StandardFixpointInterpreter`] with the default self-dependent index.
pub type SimpleFixpointInterpreter<I, P, Store> = StandardFixpointInterpreter<
    I,
    P,
    Store,
    OwnerSummaryDeps<<P as FixpointProfile<I>>::SummaryKey>,
>;

impl<I, P, Store, Deps> StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Build a driver wrapping `inner`, with an explicit dependency index.
    pub fn with_dependency_index(
        inner: I,
        store: Store,
        strategy: <P::Summary as Summary>::Strategy,
        deps: Deps,
    ) -> Self {
        Self {
            inner,
            summaries: HashMap::new(),
            store,
            deps,
            worklist: VecDeque::new(),
            frame_stack: Vec::new(),
            current_owner: None,
            pending_effects: Vec::new(),
            phase: FixpointPhase::Join,
            strategy,
        }
    }

    /// The wrapped interpreter.
    pub fn inner(&self) -> &I {
        &self.inner
    }

    /// The wrapped interpreter, mutably.
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.inner
    }

    /// Consume the driver, returning the wrapped interpreter.
    pub fn into_inner(self) -> I {
        self.inner
    }

    /// The analysis-chosen side store (e.g. block-boundary facts or `()`).
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The analysis-chosen side store, mutably.
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// The dependency index.
    pub fn dependency_index(&self) -> &Deps {
        &self.deps
    }

    /// The dependency index, mutably (register edges before solving).
    pub fn dependency_index_mut(&mut self) -> &mut Deps {
        &mut self.deps
    }

    /// The current convergence phase.
    pub fn phase(&self) -> FixpointPhase {
        self.phase
    }

    /// Set the convergence phase.
    pub fn set_phase(&mut self, phase: FixpointPhase) {
        self.phase = phase;
    }

    /// The current summary for `owner`, if any.
    pub fn summary(&self, owner: &P::SummaryKey) -> Option<&P::Summary> {
        self.summaries.get(owner)
    }

    /// The current summary for `owner`, mutably, if present.
    ///
    /// For analyses (e.g. the forward engine) whose per-owner merge is driven by
    /// an analysis policy rather than the context-free [`Summary::merge`], and so
    /// update summary storage directly.
    pub fn summary_mut(&mut self, owner: &P::SummaryKey) -> Option<&mut P::Summary> {
        self.summaries.get_mut(owner)
    }

    /// Direct mutable access to the summary table (insert/get-or-insert for
    /// analyses that seed owners with call-site metadata not derivable from the
    /// owner key alone).
    pub fn summaries_mut(&mut self) -> &mut HashMap<P::SummaryKey, P::Summary> {
        &mut self.summaries
    }

    /// All converged summaries.
    pub fn summaries(&self) -> &HashMap<P::SummaryKey, P::Summary> {
        &self.summaries
    }

    /// Enqueue `owner` for (re)analysis.
    pub fn schedule(&mut self, owner: P::SummaryKey) {
        self.worklist.push_back(WorkItem::Analyze(owner));
    }

    /// The current frame stack (empty except mid-`run_frame`).
    pub fn frame_stack(&self) -> &[P::Frame] {
        self.frame_stack.as_slice()
    }

    /// Drop any frames left on the stack.
    pub fn clear_frame_stack(&mut self) {
        self.frame_stack.clear();
    }

    /// The owner currently being analysed, if any.
    pub fn current_owner(&self) -> Option<&P::SummaryKey> {
        self.current_owner.as_ref()
    }

    /// Queue a summary effect to be applied after the current owner completes.
    pub fn push_summary_effect(&mut self, effect: SummaryEffect<P::SummaryKey, P::Summary>) {
        self.pending_effects.push(effect);
    }
}

impl<I, P, Store> StandardFixpointInterpreter<I, P, Store, OwnerSummaryDeps<P::SummaryKey>>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Build a driver wrapping `inner`, with the default self-dependent index.
    pub fn new(inner: I, store: Store, strategy: <P::Summary as Summary>::Strategy) -> Self {
        Self::with_dependency_index(inner, store, strategy, OwnerSummaryDeps::new())
    }
}
