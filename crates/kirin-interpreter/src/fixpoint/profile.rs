//! The fixpoint profile: the owner-summary type family.
//!
//! [`FixpointProfile`] bundles the four owner-summary types the driver needs so
//! call sites don't spell them out individually. Crucially it is parameterised by
//! the wrapped interpreter `I` and does **not** re-declare `I`'s associated types
//! ([`Value`](crate::Interp::Value) / [`Error`](crate::Interp::Error) /
//! [`Effect`](crate::Interp::Effect) / [`Kind`](crate::Interp::Kind)): the
//! interpreter stays the single source of those. A profile owns only the
//! convergence vocabulary.

use std::hash::Hash;

use crate::Interp;

use super::Summary;

/// The owner-summary types a [`StandardFixpointInterpreter`](super::StandardFixpointInterpreter)
/// converges over the interpreter `I`.
///
/// `SummaryKey` identifies a summary *owner* (a dataflow equation — a
/// function+context, a block); it is **not** a
/// [`LatticeAnchor`](crate::LatticeAnchor), which identifies fact *locations*
/// inside a summary or store.
pub trait FixpointProfile<I: Interp> {
    /// Key identifying a summary owner in the worklist.
    type SummaryKey: Clone + Eq + Hash;
    /// The per-owner summary the driver converges.
    type Summary: Summary;
    /// The frame type pushed to analyse one owner.
    type Frame;
    /// The completion an owner's frame run produces.
    type Completion;
}
