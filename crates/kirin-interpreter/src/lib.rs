//! Interpreter framework for Kirin IR.
//!
//! # Framework shape
//!
//! - **Shared framework** ([`Interp`],
//!   [`Interpretable`], [`Frame`]/[`FrameEngine`]/[`FrameEffect`]/[`drive_frames`],
//!   [`StandardFixpointInterpreter`]): the engine trait, the direction-neutral
//!   frame driver loop, and the owner-summary fixpoint driver.
//! - **Semantics vs shape** ([`semantics`]): statement rules are selected by a
//!   compile-time [`SemanticKey`] — [`ForwardEval`], [`StrongDemand`],
//!   [`ClassicLiveness`], or a downstream key — naming *what* a rule means.
//!   Each key declares the [`AnalysisShape`] its solver runs on
//!   ([`SparseForwardShape`], [`SparseBackwardShape`], [`DenseForwardShape`],
//!   [`DenseBackwardShape`]) — the *mechanics*: anchoring, direction, store,
//!   and fixpoint discipline — and joins that shape's *family*
//!   ([`SparseForwardSemantic`] et al.), which is what the generic engines
//!   bound their key parameter with. One dialect type carries one
//!   [`Interpretable`] rule per key, and two keys may share one shape, without
//!   coherence conflicts.
//! - **[`SparseForwardShape`] engines** ([`SparseForwardInterp`], [`Env`],
//!   [`SparseForwardEffect`]; [`ConcreteInterpreter`], and
//!   [`SparseForwardInterpreter`]`<..., Sem = ForwardEval>`):
//!   [`ForwardEval`] — concrete execution, constant propagation, interval
//!   analysis (the value domain, not the key, distinguishes them).
//! - **[`SparseBackwardShape`] engine** (shape-generic [`SparseBackwardInterp`]
//!   with [`StrongDemand`]'s helper [`DemandInterp`], [`SparseBackwardEffect`],
//!   [`SparseBackwardInterpreter`]`<..., Sem = StrongDemand>`): strong (true)
//!   liveness, one fact per SSA value, propagated value-by-value along def
//!   links.
//! - **[`DenseBackwardShape`] engine** (shape-generic [`DenseBackwardInterp`]
//!   with [`ClassicLiveness`]'s helper [`ClassicLivenessInterp`],
//!   [`DenseBackwardEffect`],
//!   [`DenseBackwardInterpreter`]`<..., Sem = ClassicLiveness>`):
//!   per-program-point liveness with block-boundary set summaries plus
//!   on-demand per-point reconstruction.
//!
//! # Two-persona contract
//!
//! - **Dialect authors** implement [`Interpretable<I, Semantics>`](Interpretable)
//!   per semantic key (and [`FunctionEntry`] for callable statements). A rule
//!   receives the engine `interp` directly. Shape-generic mechanics live on
//!   the engine traits (read/write on [`SparseForwardInterp`];
//!   fact/raise-fact on [`SparseBackwardInterp`]; opaque point-state access on
//!   [`DenseBackwardInterp`]); semantics-specific vocabulary lives in helper
//!   traits — demand rules bind [`DemandInterp`]
//!   (`demand`/`is_demanded`/`demand_uses_if_observable`), classic-liveness rules bind
//!   [`ClassicLivenessInterp`] (`gen_live`/`kill_def`/`gen_uses_kill_defs`).
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

mod core;
mod engines;
mod facts;
mod fixpoint;
mod semantics;

// The shared chassis: engine trait + dialect dispatch, effect types,
// activation storage, calling conventions, errors, and IR queries.
pub use self::core::{
    AbstractInterpreter, Env, GraphWalkPlan, Interp, InterpLocation, SparseForwardInterp,
};
pub use self::core::{Body, CallEffect, CallableBody, Callee, Edge, SparseForwardEffect};
pub use self::core::{BranchCondition, HasProductValue, expect_single};
pub use self::core::{CrossStageLinker, FunctionTarget, Linker, SameStageLinker};
pub use self::core::{EnvIndex, EnvStackStore, Store};
pub use self::core::{FunctionEntry, InterpDispatch, Interpretable};
pub use self::core::{InterpreterError, StageQuery, TerminatorArgs};
// The shared, direction-neutral frame protocol: `Frame`/`FrameEffect`/
// `drive_frames` (the frame-stack driver loop) anchored on `FrameEngine`, the
// minimal engine contract. On top of it, the forward engine capabilities a frame
// can require: one narrowly scoped component trait per kind of traversal
// (`StatementDispatch`, `BlockQueries`, `CFGQueries`, `DiGraphQueries`,
// `CallServices`), so a member frame bounds only what it consumes, plus two
// whole-universe umbrellas — `ForwardFrameEngine` (full standard concrete
// surface) and `ForwardDataflowFrameEngine` (standard forward-abstract surface).
pub use self::core::{
    BlockQueries, CFGQueries, CallServices, DiGraphQueries, ForwardDataflowFrameEngine,
    ForwardFrameEngine, Frame, FrameEffect, FrameEngine, StatementDispatch, drive_frames,
};

// Concrete execution engine + the concrete standard frames: the
// representation walkers (`BlockFrame`/`CFGFrame`/`DiGraphFrame` — `UnGraph`
// traversal is a dialect/compiler call-body traversal) and the `CallFrame`
// call boundary.
pub use engines::concrete::{
    BlockFrame, BodyFrameEntry, CFGFrame, CallBodyTraversal, CallFrame, CallRequest, Completion,
    ConcreteInterpreter, ConcreteInterpreterCore, DefaultCallBodyTraversal, DiGraphFrame,
};
// Sparse forward engine (`Sem = ForwardEval`) + the abstract standard frames.
pub use engines::sparse_forward::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractDiGraphFrame, CallContext,
    ContextInsensitive, Owner, SparseForwardInterpreter, SparseForwardTransfer,
    StandardAbstractFrame, WideningStrategy,
};
// Sparse backward engine (`Sem = StrongDemand`).
pub use engines::sparse_backward::{
    BackwardAnalysisState, BodyScope, DemandFrame, DemandInterp, DemandSummary,
    SparseBackwardDriver, SparseBackwardEffect, SparseBackwardInterp, SparseBackwardInterpreter,
    SparseBackwardProfile, SparseBackwardTransfer,
};
// Dense backward engine (`Sem = ClassicLiveness`) + the dense standard frames.
pub use engines::dense_backward::{
    BlockLiveness, ClassicLivenessInterp, DenseBackwardCompletion, DenseBackwardDriver,
    DenseBackwardEffect, DenseBackwardFrameEngine, DenseBackwardInterp, DenseBackwardInterpreter,
    DenseBackwardProfile, DenseBackwardState, DenseBackwardTransfer, DenseBlockFrame,
    DenseBlockMode, PointFacts, SuccessorEdge,
};

// Lattice anchors (*where* facts attach), scope qualification, and the
// polymorphic fact stores. Anchor family is a property of the solver shape;
// dispatch meaning lives in `semantics`.
pub use facts::{
    Change, FactStore, LatticeAnchor, ProgramPoint, Scoped, ScopedSparseStore, SparseStore,
};

// Semantic keys (*what* a rule means — the `Interpretable`/`Interp::Semantics`
// dispatch tags) and the solver shapes each key runs on.
pub use semantics::{
    AnalysisShape, ClassicLiveness, DenseBackwardSemantic, DenseBackwardShape,
    DenseForwardSemantic, DenseForwardShape, ForwardEval, SemanticKey, SparseBackwardSemantic,
    SparseBackwardShape, SparseForwardSemantic, SparseForwardShape, StrongDemand,
};

// The owner-summary fixpoint framework: a [`StandardFixpointInterpreter`] wraps
// any [`Interp`] and drives it to an owner-summary fixpoint (one work item per
// owner, intra-owner traversal on the frame stack, inter-owner convergence via a
// pluggable [`SummaryDependencyIndex`]). The wrapped interpreter stays the single
// source of value/error/effect/semantics; a [`FixpointProfile`] adds only the
// owner-summary types.
pub use fixpoint::{
    BackwardSummaryDeps, FixpointPhase, FixpointProfile, ForwardSummaryDeps, OwnerSemantics,
    OwnerSummaryDeps, SimpleFixpointInterpreter, StandardFixpointInterpreter, Summary,
    SummaryDependencies, SummaryDependency, SummaryDependencyIndex, SummaryEffect, WorkItem,
};

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::{FunctionEntry, InterpDispatch, Interpretable};

/// Everything a dialect author needs to implement statement semantics —
/// forward evaluation (`Interpretable<I, ForwardEval>`), backward demand
/// (`Interpretable<I, StrongDemand>`), classic per-point liveness
/// (`Interpretable<I, ClassicLiveness>`), and downstream semantic keys
/// (`impl SemanticKey for MyKey { type Shape = ...; }`).
pub mod dialect {
    pub use crate::{
        AnalysisShape, Body, BranchCondition, CallEffect, CallableBody, Callee, ClassicLiveness,
        ClassicLivenessInterp, DemandInterp, DenseBackwardEffect, DenseBackwardInterp,
        DenseBackwardShape, DenseForwardShape, Edge, ForwardEval, FunctionEntry, HasProductValue,
        Interp, Interpretable, InterpreterError, PointFacts, SemanticKey, SparseBackwardEffect,
        SparseBackwardInterp, SparseBackwardShape, SparseForwardEffect, SparseForwardInterp,
        SparseForwardShape, StrongDemand, SuccessorEdge,
    };
}

/// Everything a compiler author needs to run engines or customize traversal.
pub mod engine {
    pub use crate::{
        AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractDiGraphFrame,
        AbstractInterpreter, BlockFrame, BlockQueries, BodyFrameEntry, CFGFrame, CFGQueries,
        CallBodyTraversal, CallContext, CallFrame, CallRequest, CallServices, Callee, Completion,
        ConcreteInterpreter, ConcreteInterpreterCore, ContextInsensitive, CrossStageLinker,
        DefaultCallBodyTraversal, DenseBackwardCompletion, DenseBackwardFrameEngine,
        DenseBackwardInterp, DenseBackwardInterpreter, DenseBackwardState, DenseBlockFrame,
        DiGraphFrame, DiGraphQueries, Env, ForwardDataflowFrameEngine, ForwardFrameEngine, Frame,
        FrameEffect, FrameEngine, FunctionTarget, Interp, InterpDispatch, InterpreterError, Linker,
        SameStageLinker, SparseBackwardInterp, SparseBackwardInterpreter, SparseForwardInterp,
        SparseForwardInterpreter, StandardAbstractFrame, StatementDispatch, WideningStrategy,
        drive_frames, expect_single,
    };
}
