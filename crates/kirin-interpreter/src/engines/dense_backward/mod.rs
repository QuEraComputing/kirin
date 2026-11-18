//! The dense backward engine (`Sem = ClassicLiveness` by default): per-point
//! facts over reverse block walks, joined into one environment per analyzed
//! callable.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{DenseBlockFrame, DenseBlockMode};
pub use interp::{
    ClassicLivenessInterp, DenseBackwardCompletion, DenseBackwardDriver, DenseBackwardEffect,
    DenseBackwardFrameEngine, DenseBackwardInterp, DenseBackwardInterpreter, DenseBackwardProfile,
    DenseBackwardTransfer, PointFacts, SuccessorEdge,
};
