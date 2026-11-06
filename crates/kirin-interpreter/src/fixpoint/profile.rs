//! The fixpoint profile: the work-item type family.
//!
//! [`FixpointProfile`] bundles the types the driver needs so call sites don't
//! spell them out individually. Crucially it is parameterised by the wrapped
//! interpreter `I` and does **not** re-declare `I`'s associated types
//! ([`Value`](crate::Interp::Value) / [`Error`](crate::Interp::Error) /
//! [`Effect`](crate::Interp::Effect) / [`Semantics`](crate::Interp::Semantics)): the
//! interpreter stays the single source of those. A profile owns only the
//! worklist vocabulary.

use std::hash::Hash;

use crate::Interp;

/// The types a [`StandardFixpointInterpreter`](super::StandardFixpointInterpreter)
/// iterates over the interpreter `I`.
///
/// `Work` identifies one unit of re-analysis (a block walk, a statement, a
/// graph); it is **not** a [`LatticeAnchor`](crate::LatticeAnchor), which
/// identifies fact *locations* inside a store.
pub trait FixpointProfile<I: Interp> {
    /// One item of the worklist.
    type Work: Clone + Eq + Hash;
    /// The frame type pushed to analyse one work item.
    type Frame;
    /// The completion a work item's frame run produces.
    type Completion;
}
