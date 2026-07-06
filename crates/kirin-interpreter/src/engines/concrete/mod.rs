//! Concrete execution: single-path evaluation over runtime values.

pub(crate) mod frames;
pub(crate) mod interp;

pub use frames::{BodyFrame, CallFrame, Completion, FrameBuild, StandardFrame};
pub use interp::ConcreteInterpreter;
