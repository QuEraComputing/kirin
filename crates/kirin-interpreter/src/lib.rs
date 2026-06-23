//! Interpreter framework for Kirin IR.
//!
//! The framework is organized around a two-persona contract:
//!
//! - **Dialect authors** implement [`Interpretable`] (and [`FunctionEntry`]
//!   for callable statements), specialized on a **context type** — the forward
//!   context [`ForwardContext`] for execution/abstract interpretation. They see three
//!   concepts: the context API ([`ForwardCtx`] read/write on [`ForwardContext`]), the closed
//!   [`ForwardEffect`] algebra they return, and plain value-domain bounds on
//!   `I::Value`. A dialect with structured control runs a sub-computation by
//!   [pushing a frame](ForwardEffect::Push) it owns — there is no framework
//!   "scope". A future analysis (e.g. liveness) is a *distinct* context type, so
//!   its rules never overlap the forward ones — the context type, not the engine
//!   type, is the specialization boundary.
//! - **Compiler authors** compose languages into stage enums (deriving
//!   [`InterpDispatch`] alongside `StageMeta`) and run engines:
//!   [`ConcreteInterpreter`] for execution, [`AbstractInterpreter`] for
//!   lattice-based fixpoint analysis. Calling conventions are [`Linker`]
//!   components passed by value — [`SameStageLinker`] (default) or
//!   [`CrossStageLinker`] for multi-language pipelines. A language that adds a
//!   structured-control dialect composes its own total frame type embedding the
//!   standard frames plus the dialect's frames.
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

pub use abstract_interp::{AbstractInterpreter, CallContext, ContextInsensitive, WideningStrategy};
pub use concrete::ConcreteInterpreter;
pub use ctx::{ForwardContext, ForwardCtx, ForwardInterp, Interp, InterpretCtx};
pub use dispatch::{FunctionEntry, InterpDispatch, Interpretable};
pub use effect::{CallEffect, Callee, Edge, ForwardEffect, FunctionBody};
pub use env::{Env, EnvIndex, EnvStackStore};

pub use error::InterpreterError;
// The shared frame protocol (concrete + abstract implement it).
pub use frame::{AbstractFrameDriver, Frame, FrameDriver, FrameEffect, FrameEngine, drive_frames};
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
/// A forward statement rule is `impl<I: ForwardInterp, ..> Interpretable<ForwardContext<'_, I>> for Op`,
/// reading/writing through [`ForwardContext`]'s [`ForwardCtx`] helpers and returning
/// `I::Effect` (the forward control algebra [`ForwardEffect`]).
pub mod dialect {
    pub use crate::{
        BranchCondition, CallEffect, Callee, Edge, ForwardContext, ForwardCtx, ForwardEffect,
        ForwardInterp, FunctionBody, FunctionEntry, HasProductValue, Interp, InterpretCtx,
        Interpretable, InterpreterError,
    };
}

/// Everything a compiler author needs to run interpreters and analyses.
///
/// Ordinary users need only the engines + [`Linker`] + value/error domain.
/// Advanced compiler/analysis authors customize *traversal* with a custom frame
/// type: concrete via `ConcreteInterpreter<.., F>` reusing [`BodyFrame`]/
/// [`CallFrame`] through [`FrameBuild`]; abstract via `AbstractInterpreter<.., P, F>`
/// reusing the `Abstract*Frame`s through [`AbstractFrameBuild`]. Both implement
/// the shared [`Frame`]/[`FrameDriver`] protocol ([`AbstractFrameDriver`] adds the
/// abstract-only capabilities). Analysis *policy* is [`CallContext`] + [`WideningStrategy`].
pub mod engine {
    pub use crate::{
        AbstractBlockFrame, AbstractCallFrame, AbstractCfgFrame, AbstractCompletion,
        AbstractFrameBuild, AbstractFrameDriver, AbstractFunctionFrame, AbstractInterpreter,
        BodyFrame, CallContext, CallFrame, Callee, Completion, ConcreteInterpreter,
        ContextInsensitive, CrossStageLinker, ForwardInterp, Frame, FrameBuild, FrameDriver,
        FrameEffect, FrameEngine, FunctionTarget, Interp, InterpDispatch, InterpreterError, Linker,
        SameStageLinker, StandardAbstractFrame, StandardFrame, WideningStrategy, drive_frames,
        expect_single,
    };
}
