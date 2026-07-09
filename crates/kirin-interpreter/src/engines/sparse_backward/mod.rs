//! The sparse backward engine (`Sem = StrongDemand` by default):
//! value-anchored demand propagated along SSA def links, with the driver's
//! self-dependent owner index as the demand worklist.

pub(crate) mod interp;

pub use interp::{
    BackwardAnalysisState, CfgScope, DemandFrame, DemandInterp, DemandSummary,
    SparseBackwardDriver, SparseBackwardEffect, SparseBackwardInterp, SparseBackwardInterpreter,
    SparseBackwardProfile, SparseBackwardTransfer,
};
