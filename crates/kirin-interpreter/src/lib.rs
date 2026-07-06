//! Interpreter framework for Kirin IR.
//!
//! # Framework shape
//!
//! - **Shared framework** ([`Interp`],
//!   [`Interpretable`], [`Frame`]/[`FrameEngine`]/[`FrameEffect`]/[`drive_frames`],
//!   [`StandardFixpointInterpreter`]): the engine trait, the direction-neutral
//!   frame driver loop, and the owner-summary fixpoint driver. Statement
//!   semantics are selected by a compile-time [`Kind`](Interp::Kind) marker —
//!   one dialect type carries one [`Interpretable`] rule per kind without
//!   coherence conflicts.
//! - **[`SparseForward`]** ([`SparseForwardInterp`], [`Env`],
//!   [`SparseForwardEffect`], [`ConcreteInterpreter`], [`SparseForwardInterpreter`]):
//!   forward evaluation — concrete execution, constant propagation, interval
//!   analysis.
//! - **[`SparseBackward`]** ([`SparseBackwardInterp`], [`SparseBackwardEffect`],
//!   [`SparseBackwardInterpreter`]): backward demand — strong (true) liveness,
//!   one fact per SSA value, propagated value-by-value along def links.
//! - **[`DenseBackward`]** ([`DenseBackwardInterp`], [`DenseBackwardEffect`],
//!   [`DenseBackwardInterpreter`]): classic per-program-point liveness —
//!   block-boundary set summaries plus on-demand per-point reconstruction.
//! - **Future sibling modes** (e.g. `DenseForward` typestate) each add a marker
//!   + engine trait + effect algebra without touching the existing ones.
//!
//! # Two-persona contract
//!
//! - **Dialect authors** implement [`Interpretable<I, Kind>`](Interpretable)
//!   per semantics (and [`FunctionEntry`] for callable statements). A rule
//!   receives the engine `interp` directly and uses that kind's helpers:
//!   forward rules read/write SSA values ([`SparseForwardInterp`]), demand
//!   rules read/raise demand facts ([`SparseBackwardInterp`], with
//!   `transfer_ordinary` as the purity-aware one-liner), dense rules gen/kill
//!   the point state ([`DenseBackwardInterp`], with `transfer_classic`).
//!   Structured dialects may push dialect-owned frames.
//! - **Compiler authors** compose languages into stage enums (deriving
//!   [`InterpDispatch`] alongside `StageMeta`) and run engines:
//!   [`ConcreteInterpreter`] for execution, [`SparseForwardInterpreter`] /
//!   [`SparseBackwardInterpreter`] / [`DenseBackwardInterpreter`] for analyses,
//!   and the [`AbstractInterpreter`] trait for lattice-valued engines. Calling
//!   conventions are [`Linker`] components passed by value.
//!
//! Engines interpret the same dialect rules: concrete and abstract execution
//! differ only in the value domain and in how undecided control flow
//! (cf's [`SparseForwardEffect::Branch`], or a control dialect's own pushed frame) is
//! driven.

mod anchor;
mod concrete_frames;
mod concrete_interp;
mod dense_backward_frames;
mod dense_backward_interp;
mod dispatch;
mod effect;
mod env;
mod error;
mod fixpoint;
mod frame;
mod interp;
mod linker;
mod query;
mod sparse_backward_interp;
mod sparse_forward_frames;
mod sparse_forward_interp;
mod store;
mod topology;
mod value;

pub use concrete_interp::ConcreteInterpreter;
pub use dense_backward_frames::{
    DenseBlockFrame, DenseBlockMode, DenseFrameBuild, StandardDenseBackwardFrame,
};
pub use dense_backward_interp::{
    BlockLiveness, DenseAnalysisState, DenseBackwardCompletion, DenseBackwardDriver,
    DenseBackwardEffect, DenseBackwardFrameDriver, DenseBackwardInterp, DenseBackwardInterpreter,
    DenseBackwardProfile, DenseBackwardTransfer, PointFacts, SuccessorEdge,
};
pub use dispatch::{FunctionEntry, InterpDispatch, Interpretable};
pub use effect::{CallEffect, Callee, Edge, FunctionBody, SparseForwardEffect};
pub use env::{EnvIndex, EnvStackStore, Store};
pub use interp::{AbstractInterpreter, Env, Interp, InterpLocation, SparseForwardInterp};
pub use sparse_backward_interp::{
    BackwardAnalysisState, DemandFrame, DemandSummary, RegionScope, SparseBackwardDriver,
    SparseBackwardEffect, SparseBackwardInterp, SparseBackwardInterpreter, SparseBackwardProfile,
    SparseBackwardTransfer,
};
pub use sparse_forward_interp::{
    CallContext, ContextInsensitive, Owner, SparseForwardInterpreter, SparseForwardTransfer,
    WideningStrategy,
};

pub use error::InterpreterError;
// The shared, direction-neutral frame protocol (`Frame`/`FrameEngine`/
// `FrameEffect`/`drive_frames`) plus the forward frame-driver capability surfaces.
pub use frame::{
    ForwardDataflowFrameDriver, ForwardFrameDriver, Frame, FrameEffect, FrameEngine, drive_frames,
};
// Backward-compatible aliases for the forward frame-driver capability surfaces.
pub use frame::ForwardDataflowFrameDriver as AbstractFrameDriver;
pub use frame::ForwardFrameDriver as FrameDriver;
// Concrete standard frames.
pub use concrete_frames::{BodyFrame, CallFrame, Completion, FrameBuild, StandardFrame};
// Abstract standard frames.
pub use sparse_forward_frames::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractFrameBuild,
    StandardAbstractFrame,
};
// The lattice-anchor-polymorphic dataflow vocabulary: sparse/dense anchors,
// kind markers, scope qualification, and kind-specific stores. Analyses
// converge on the owner-summary framework below ([`StandardFixpointInterpreter`]).
pub use anchor::{
    AnalysisKind, Change, DenseAnchor, DenseBackward, DenseForward, LatticeAnchor, ProgramPoint,
    Scoped, SparseBackward, SparseForward,
};
pub use linker::{CrossStageLinker, FunctionTarget, Linker, SameStageLinker};
pub use query::StageQuery;
// The owner-summary fixpoint framework: a [`StandardFixpointInterpreter`] wraps
// any [`Interp`] and drives it to an owner-summary fixpoint (one work item per
// owner, intra-owner traversal on the frame stack, inter-owner convergence via a
// pluggable [`SummaryDependencyIndex`]). The wrapped interpreter stays the single
// source of value/error/effect/kind; a [`FixpointProfile`] adds only the
// owner-summary types.
pub use fixpoint::{
    BackwardSummaryDeps, FixpointPhase, FixpointProfile, ForwardSummaryDeps, OwnerSemantics,
    OwnerSummaryDeps, SimpleFixpointInterpreter, StandardFixpointInterpreter, Summary,
    SummaryDependencies, SummaryDependency, SummaryDependencyIndex, SummaryEffect, WorkItem,
};
pub use store::{DenseBlockStore, DensePointStore, FactStore, ScopedSparseStore, SparseStore};
pub use topology::{BlockTopology, RegionTopology, region_topology};
pub use value::{BranchCondition, HasProductValue, expect_single};

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::{FunctionEntry, InterpDispatch, Interpretable};

/// Everything a dialect author needs to implement statement semantics —
/// forward evaluation (`Interpretable<I, SparseForward>`), backward demand
/// (`Interpretable<I, SparseBackward>`), and dense backward liveness
/// (`Interpretable<I, DenseBackward>`).
pub mod dialect {
    pub use crate::{
        BranchCondition, CallEffect, Callee, DenseBackward, DenseBackwardEffect,
        DenseBackwardInterp, Edge, FunctionBody, FunctionEntry, HasProductValue, Interp,
        Interpretable, InterpreterError, PointFacts, SparseBackward, SparseBackwardEffect,
        SparseBackwardInterp, SparseForward, SparseForwardEffect, SparseForwardInterp,
        SuccessorEdge,
    };
}

/// Everything a compiler author needs to run engines or customize traversal.
pub mod engine {
    pub use crate::{
        AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractFrameBuild,
        AbstractFrameDriver, AbstractInterpreter, BodyFrame, CallContext, CallFrame, Callee,
        Completion, ConcreteInterpreter, ContextInsensitive, CrossStageLinker,
        DenseBackwardCompletion, DenseBackwardFrameDriver, DenseBackwardInterp,
        DenseBackwardInterpreter, DenseBlockFrame, DenseFrameBuild, Env,
        ForwardDataflowFrameDriver, ForwardFrameDriver, Frame, FrameBuild, FrameDriver,
        FrameEffect, FrameEngine, FunctionTarget, Interp, InterpDispatch, InterpreterError, Linker,
        SameStageLinker, SparseBackwardInterp, SparseBackwardInterpreter, SparseForwardInterp,
        SparseForwardInterpreter, StandardAbstractFrame, StandardDenseBackwardFrame, StandardFrame,
        WideningStrategy, drive_frames, expect_single,
    };
}
