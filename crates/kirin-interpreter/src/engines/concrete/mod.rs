//! Concrete execution: single-path evaluation over runtime values.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{
    BlockFrame, BodyFrameEntry, CFGFrame, CallBodyTraversal, CallFrame, CallRequest, Completion,
    DefaultCallBodyTraversal, DiGraphFrame,
};
pub use interp::{ConcreteInterpreter, ConcreteInterpreterCore};
