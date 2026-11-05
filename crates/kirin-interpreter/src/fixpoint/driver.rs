//! The worklist fixpoint driver.
//!
//! [`StandardFixpointInterpreter`] wraps one [`Interp`] (`inner`) and adds the
//! outer convergence loop: a worklist of work items and an analysis-chosen side
//! store. It *is* an [`Interp`] itself (by delegation to `inner`, see
//! `delegates.rs`), so frames and dialect rules see a single interpreter. Each
//! work item's traversal runs on the frame stack via
//! [`run_frame`](Self::run_frame); the worklist runs via
//! [`drain_worklist`](Self::drain_worklist) (see `solver.rs`).

use std::collections::VecDeque;

use crate::Interp;

use super::FixpointProfile;

/// Drives a [`FixpointProfile`] over an interpreter `I` to a fixpoint.
pub struct StandardFixpointInterpreter<I, P, Store>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    pub(super) inner: I,
    pub(super) store: Store,
    pub(super) worklist: VecDeque<P::Work>,
    pub(super) frame_stack: Vec<P::Frame>,
    pub(super) current_work: Option<P::Work>,
}

impl<I, P, Store> StandardFixpointInterpreter<I, P, Store>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Build a driver wrapping `inner`, with the analysis-chosen side store.
    pub fn new(inner: I, store: Store) -> Self {
        Self {
            inner,
            store,
            worklist: VecDeque::new(),
            frame_stack: Vec::new(),
            current_work: None,
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

    /// The analysis-chosen side store (e.g. a dependency map or `()`).
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The analysis-chosen side store, mutably.
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Enqueue `work` for (re)analysis.
    pub fn schedule(&mut self, work: P::Work) {
        self.worklist.push_back(work);
    }

    /// The work item currently being analysed, if any.
    pub fn current_work(&self) -> Option<&P::Work> {
        self.current_work.as_ref()
    }
}
