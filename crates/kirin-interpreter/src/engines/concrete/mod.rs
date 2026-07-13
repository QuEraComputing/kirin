//! Concrete execution: single-path evaluation over runtime values.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{
    BlockMode, BodyFrame, CallFrame, Completion, DiGraphFrame, FrameBuild, StandardFrame,
};
pub use interp::ConcreteInterpreter;
