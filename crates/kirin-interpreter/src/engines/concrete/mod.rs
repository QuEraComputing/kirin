//! Concrete execution: single-path evaluation over runtime values.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{
    BlockFrame, CallFrame, CfgFrame, Completion, DiGraphFrame, FrameBuild, StandardFrame,
    UnGraphEntry,
};
pub use interp::ConcreteInterpreter;
