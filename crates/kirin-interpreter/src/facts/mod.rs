//! Dataflow fact vocabulary: anchors (*where* facts attach) plus the
//! polymorphic fact stores. Fixpoint clients use these, but they are dataflow
//! vocabulary, not the convergence driver itself — and not IR queries either.

pub(crate) mod anchor;
pub(crate) mod store;

pub use anchor::{Change, DenseAnchor, LatticeAnchor, ProgramPoint, Scoped};
pub use store::{DenseBlockStore, DensePointStore, FactStore, ScopedSparseStore, SparseStore};
