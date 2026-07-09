//! Interpreter framework for Kirin IR.
//!
//! # Framework shape
//!
//! - **Shared framework** ([`Interp`],
//!   [`Interpretable`], [`Frame`]/[`FrameEngine`]/[`FrameEffect`]/[`drive_frames`]):
//!   the engine trait plus the direction-neutral frame driver loop. Statement
//!   semantics are selected by a compile-time [`Kind`](Interp::Kind) marker —
//!   [`ForwardEval`] today (covering concrete execution, constant propagation,
//!   and interval analysis), [`BackwardLiveness`] reserved for the future.
//! - **Forward specialization** ([`ForwardEvalInterp`], [`Env`],
//!   [`ForwardEffect`], [`ConcreteInterpreter`], [`ForwardAbstractInterpreter`]):
//!   concrete execution and forward lattice analysis.
//! - **Future sibling modes** would each add a marker + engine trait without
//!   touching `ForwardEval`: `ForwardType`/`ForwardTypeInterp` (type inference),
//!   `BackwardDataflow`/`BackwardDataflowInterp`, and
//!   [`BackwardLiveness`]/`BackwardLivenessInterp` — each defining its own
//!   effect/result, fact store, and frame-driver capability while reusing the
//!   shared framework.
//!
//! # Two-persona contract
//!
//! - **Dialect authors** implement [`Interpretable<I, ForwardEval>`](Interpretable)
//!   (and [`FunctionEntry`] for callable statements). A rule receives the engine
//!   `interp` directly, reads/writes SSA values through the
//!   [`ForwardEvalInterp`] helpers (`interp.read`/`interp.write`), and returns
//!   [`ForwardEffect`]. Structured dialects may push dialect-owned frames.
//! - **Compiler authors** compose languages into stage enums (deriving
//!   [`InterpDispatch`] alongside `StageMeta`) and run engines:
//!   [`ConcreteInterpreter`] for execution, [`ForwardAbstractInterpreter`] for
//!   forward lattice-based fixpoint analysis, and the [`AbstractInterpreter`]
//!   trait for lattice-valued engines. Calling conventions are [`Linker`]
//!   components passed by value.
//!
//! Engines interpret the same dialect rules: concrete and abstract execution
//! differ only in the value domain and in how undecided control flow
//! (cf's [`ForwardEffect::Branch`], or a control dialect's own pushed frame) is
//! driven.

mod concrete_frames;
mod concrete_interp;
mod dispatch;
mod effect;
mod env;
mod error;
mod forward_abstract_frames;
mod forward_abstract_interp;
mod frame;
mod interp;
mod linker;
mod query;
mod value;

pub use forward_abstract_interp::{
    CallContext, ContextInsensitive, ForwardAbstractInterpreter, WideningStrategy,
};
pub use concrete_interp::ConcreteInterpreter;
pub use interp::{
    AbstractInterpreter, BackwardLiveness, Env, ForwardEval, ForwardEvalInterp, Interp,
    InterpLocation,
};
pub use dispatch::{FunctionEntry, InterpDispatch, Interpretable};
pub use effect::{CallEffect, Callee, Edge, ForwardEffect, FunctionBody};
pub use env::{EnvIndex, EnvStackStore, Store};

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
pub use forward_abstract_frames::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCfgFrame, AbstractCompletion,
    AbstractFrameBuild, AbstractFunctionFrame, StandardAbstractFrame,
};
pub use linker::{CrossStageLinker, FunctionTarget, Linker, SameStageLinker};
pub use query::StageQuery;
pub use value::{BranchCondition, HasProductValue, expect_single};

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::{FunctionEntry, InterpDispatch, Interpretable};

/// Everything a dialect author needs to implement interpretation.
///
/// Everything a dialect author needs to implement forward statement semantics.
pub mod dialect {
    pub use crate::{
        BackwardLiveness, BranchCondition, CallEffect, Callee, Edge, ForwardEffect, ForwardEval,
        ForwardEvalInterp, FunctionBody, FunctionEntry, HasProductValue, Interp, Interpretable,
        InterpreterError,
    };
}

/// Everything a compiler author needs to run engines or customize traversal.
pub mod engine {
    pub use crate::{
        AbstractBlockFrame, AbstractCallFrame, AbstractCfgFrame, AbstractCompletion,
        AbstractFrameBuild, AbstractFrameDriver, AbstractFunctionFrame, AbstractInterpreter,
        BodyFrame, CallContext, CallFrame, Callee, Completion, ConcreteInterpreter,
        ContextInsensitive, CrossStageLinker, Env, ForwardAbstractInterpreter,
        ForwardDataflowFrameDriver, ForwardEvalInterp, ForwardFrameDriver, Frame, FrameBuild,
        FrameDriver, FrameEffect, FrameEngine, FunctionTarget, Interp, InterpDispatch,
        InterpreterError, Linker, SameStageLinker, StandardAbstractFrame, StandardFrame,
        WideningStrategy, drive_frames, expect_single,
    };
}
