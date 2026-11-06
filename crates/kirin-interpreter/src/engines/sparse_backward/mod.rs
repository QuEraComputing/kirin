//! The sparse backward engine (`Sem = StrongDemand` by default):
//! value-anchored demand in one environment per callable context, propagated
//! along SSA def links and across call boundaries by statement work items.

mod frames;
pub(crate) mod interp;

pub use frames::DemandFrame;
pub use interp::{
    BodyScope, DemandContext, DemandInterp, SparseBackwardDriver, SparseBackwardEffect,
    SparseBackwardInterp, SparseBackwardInterpreter, SparseBackwardProfile, SparseBackwardTransfer,
};
