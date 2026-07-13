//! Dataflow fact vocabulary: anchors (*where* facts attach), the polymorphic
//! fact stores, and CFG topology enumeration. Fixpoint clients use these,
//! but they are dataflow vocabulary, not the convergence driver itself.

pub(crate) mod anchor;
pub(crate) mod store;
pub(crate) mod topology;

pub use anchor::{Change, DenseAnchor, LatticeAnchor, ProgramPoint, Scoped};
pub use store::{DenseBlockStore, DensePointStore, FactStore, ScopedSparseStore, SparseStore};
pub use topology::{
    BlockTopology, BodyTopology, CfgTopology, GraphTopology, PortBoundary, body_topology,
    cfg_topology,
};
