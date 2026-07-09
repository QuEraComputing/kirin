//! Concrete clients of the shared framework: one engine per analysis shape,
//! each composing the core chassis, the fact vocabulary, and the fixpoint
//! driver. Engines are generic over their shape's semantic family with the
//! canonical key as default.

pub(crate) mod concrete;
pub(crate) mod dense_backward;
pub(crate) mod sparse_backward;
pub(crate) mod sparse_forward;
