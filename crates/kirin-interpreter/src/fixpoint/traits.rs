//! Owner-summary vocabulary for the fixpoint driver.
//!
//! The driver ([`StandardFixpointInterpreter`](super::StandardFixpointInterpreter))
//! runs one work item per **summary owner** and delegates intra-owner traversal
//! to a frame stack. These traits describe only the convergence boundary:
//! [`Summary`] (the facts at an owner, plus how they merge), [`OwnerAnalysis`]
//! (how to seed, enter, and complete an owner), and [`SummaryEffect`] (the summary updates an owner produces). The
//! value/error/effect/semantics of the analysis stay on the wrapped
//! [`Interp`](crate::Interp).

/// The per-owner fact bundle the driver converges.
///
/// [`merge`](Summary::merge) folds a freshly computed `candidate` into `self`
/// and returns whether it changed, driving dependency re-scheduling.
pub trait Summary: Clone {
    fn merge(&mut self, candidate: Self) -> bool;
}

/// How the driver seeds, enters, and completes one summary owner.
///
/// `I` is the wrapped interpreter (the driver passes itself, so the owner analysis
/// can use the interpreter's env/query helpers); `K`/`S`/`F`/`C` are the profile's
/// owner key / summary / frame / completion; `E` is the interpreter's error.
pub trait OwnerAnalysis<I, K, S, F, C, E>
where
    S: Summary,
{
    /// The initial (bottom) summary for a freshly discovered owner.
    fn bottom_summary(&mut self, interp: &mut I, owner: &K) -> Result<S, E>;

    /// The root frame that analyses `owner`, given its current summary.
    fn entry_frame(&mut self, interp: &mut I, owner: &K, summary: &S) -> Result<F, E>;

    /// Turn the completion of `owner`'s frame run into summary updates.
    fn complete_owner(
        &mut self,
        interp: &mut I,
        owner: K,
        completion: C,
    ) -> Result<SummaryEffect<K, S>, E>;
}

/// The summary updates an owner analysis produces when it completes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryEffect<K, S> {
    /// No summary changed.
    None,
    /// Merge `candidate` into `owner`'s summary.
    Update { owner: K, candidate: S },
    /// Apply several `(owner, candidate)` merges in order.
    Many(Vec<(K, S)>),
}

/// One unit of the owner worklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkItem<K> {
    /// (Re)analyse the given owner.
    Analyze(K),
}
