//! Dataflow fact vocabulary: anchors and locations (*where* facts attach)
//! plus the polymorphic fact stores. Fixpoint clients use these, but they are
//! dataflow vocabulary, not the convergence driver itself — and not IR
//! queries either: the body shape an analysis enumerates before it runs is
//! [`core::topology`](crate::core::topology).

pub(crate) mod anchor;
pub(crate) mod store;

pub use anchor::{Change, DenseAnchor, LatticeAnchor, PortBoundary, ProgramPoint, Scoped};
pub use store::{DenseBlockStore, DensePointStore, FactStore, ScopedSparseStore, SparseStore};
