//! The sparse backward engine (`Sem = StrongDemand` by default):
//! value-anchored demand propagated along SSA def links and across call
//! boundaries, with the driver's self-dependent owner index as the demand
//! worklist.

mod frames;
pub(crate) mod interp;

pub use frames::DemandFrame;
pub use interp::{
    BodyScope, DemandContext, DemandInterp, DemandSummary, SparseBackwardDriver,
    SparseBackwardEffect, SparseBackwardInterp, SparseBackwardInterpreter, SparseBackwardProfile,
    SparseBackwardTransfer,
};
