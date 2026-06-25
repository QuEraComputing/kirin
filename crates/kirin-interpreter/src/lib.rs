//! Interpreter framework for Kirin IR.
//!
//! # Framework shape
//!
//! - **Shared framework** ([`Interp`], [`InterpretCtx`],
//!   [`Interpretable`], [`Frame`]/[`FrameEngine`]/[`FrameEffect`]/[`drive_frames`]):
//!   engine/context traits plus the direction-neutral frame driver loop.
//! - **Forward specialization** ([`ForwardContext`], [`Env`],
//!   [`ForwardEffect`], [`ConcreteInterpreter`], [`ForwardAbstractInterpreter`]):
//!   concrete execution and forward lattice analysis.
//! - **Future backward specialization**: expected to define its own context,
//!   effect/result, fact store, and frame-driver capability while reusing the
//!   shared framework.
//!
//! # Two-persona contract
//!
//! - **Dialect authors** implement [`Interpretable`] (and [`FunctionEntry`] for
//!   callable statements) against a context type. Forward rules use
//!   [`ForwardContext`] and return [`ForwardEffect`]. Structured dialects may push
//!   dialect-owned frames.
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

mod abstract_frame;
mod abstract_interp;
mod concrete;
mod concrete_frame;
mod ctx;
mod dispatch;
mod effect;
mod env;
mod error;
mod frame;
mod linker;
mod query;
mod value;

pub use abstract_interp::{
    CallContext, ContextInsensitive, ForwardAbstractInterpreter, WideningStrategy,
};
pub use concrete::ConcreteInterpreter;
pub use ctx::{AbstractInterpreter, Env, ForwardContext, ForwardInterp, Interp, InterpretCtx};
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
pub use concrete_frame::{BodyFrame, CallFrame, Completion, FrameBuild, StandardFrame};
// Abstract standard frames.
pub use abstract_frame::{
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
        BranchCondition, CallEffect, Callee, Edge, ForwardContext, ForwardEffect, ForwardInterp,
        FunctionBody, FunctionEntry, HasProductValue, Interp, InterpretCtx, Interpretable,
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
        ForwardDataflowFrameDriver, ForwardFrameDriver, ForwardInterp, Frame, FrameBuild,
        FrameDriver, FrameEffect, FrameEngine, FunctionTarget, Interp, InterpDispatch,
        InterpreterError, Linker, SameStageLinker, StandardAbstractFrame, StandardFrame,
        WideningStrategy, drive_frames, expect_single,
    };
}
