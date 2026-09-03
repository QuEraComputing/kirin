//! The dense backward engine (`Sem = ClassicLiveness` by default): per-point
//! facts over reverse block walks, with block-boundary summaries and on-demand
//! per-point reconstruction.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{DenseBlockFrame, DenseBlockMode, DenseFrameBuild, StandardDenseBackwardFrame};
pub use interp::{
    BlockLiveness, ClassicLivenessInterp, DenseBackwardCompletion, DenseBackwardDriver,
    DenseBackwardEffect, DenseBackwardFrameEngine, DenseBackwardInterp, DenseBackwardInterpreter,
    DenseBackwardProfile, DenseBackwardState, DenseBackwardTransfer, PointFacts, SuccessorEdge,
};
