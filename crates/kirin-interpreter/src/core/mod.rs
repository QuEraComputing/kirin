//! The shared interpreter chassis: the engine trait ([`Interp`]) and dialect
//! dispatch ([`Interpretable`]), effect types, the direction-neutral frame
//! protocol, activation storage, calling conventions, errors, and the IR
//! queries ([`query`], [`topology`]) engines run against a stage.
//! Everything here is engine-agnostic; the engines compose these pieces.

pub(crate) mod dispatch;
pub(crate) mod effect;
pub(crate) mod env;
pub(crate) mod error;
pub(crate) mod frame;
pub(crate) mod interp;
pub(crate) mod linker;
pub(crate) mod query;
pub(crate) mod topology;
pub(crate) mod value;

pub use dispatch::{FunctionEntry, InterpDispatch, Interpretable};
pub use effect::{Body, CallEffect, CallableBody, Callee, Edge, SparseForwardEffect};
pub use env::{EnvIndex, EnvStackStore, Store};
pub use error::InterpreterError;
pub use frame::{
    ForwardDataflowFrameDriver, ForwardFrameDriver, Frame, FrameEffect, FrameEngine, drive_frames,
};
pub use interp::{AbstractInterpreter, Env, Interp, InterpLocation, SparseForwardInterp};
pub use linker::{CrossStageLinker, FunctionTarget, Linker, SameStageLinker};
pub use query::{GraphWalkPlan, StageQuery};
pub use topology::{BlockTopology, BodyTopology, GraphTopology, body_topology};
pub use value::{BranchCondition, HasProductValue, expect_single};
