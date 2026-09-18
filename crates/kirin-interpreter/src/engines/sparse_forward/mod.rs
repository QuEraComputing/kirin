//! The sparse forward abstract engine (`Sem = ForwardEval` by default):
//! constant propagation, interval analysis — the value domain, not the key,
//! distinguishes them.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractDiGraphFrame,
    AbstractFrameBuild, StandardAbstractFrame,
};
pub use interp::{
    CallContext, ContextInsensitive, Owner, SparseForwardInterpreter, SparseForwardTransfer,
    WideningStrategy,
};
