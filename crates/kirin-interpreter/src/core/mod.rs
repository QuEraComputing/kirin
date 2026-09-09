//! The shared interpreter chassis: the engine trait ([`Interp`]) and dialect
//! dispatch ([`Interpretable`]), effect types, the direction-neutral frame
//! protocol, environments ([`env`]), calling conventions ([`linker`]), errors,
//! and the IR queries ([`query`]) engines run against a stage.
//! Everything here is engine-agnostic; the engines compose these pieces.

pub(crate) mod dispatch;
pub(crate) mod effect;
pub(crate) mod env;
pub(crate) mod error;
pub(crate) mod frame;
pub(crate) mod interp;
pub(crate) mod linker;
pub(crate) mod query;
pub(crate) mod value;

pub use dispatch::{InterpDispatch, Interpretable};
pub use effect::{CallEffect, Callee, Edge, SparseForwardEffect};
pub use env::{Env, EnvIndex, EnvStore, SSABinding};
pub use error::InterpreterError;
pub use frame::{
    BlockQueries, CFGQueries, CallServices, DiGraphQueries, ForwardDataflowFrameEngine,
    ForwardFrameEngine, Frame, FrameEffect, FrameEngine, StatementDispatch, drive_frames,
};
pub use interp::{AbstractInterpreter, Interp, InterpLocation, SparseForwardInterp};
pub use linker::{CrossStageLinker, LinkTarget, Linker, ResolvedCallable, SameStageLinker};
pub use query::{GraphWalkPlan, StageQuery, TerminatorArgs};
pub use value::{BranchCondition, HasProductValue, expect_single};
