//! Interpreter framework for Kirin IR.
//!
//! The framework is organized around a two-persona contract:
//!
//! - **Dialect authors** implement [`Interpretable`] (and [`FunctionEntry`]
//!   for callable statements). They see three concepts: the interpreter
//!   context [`Interp`]/[`Ctx`], the closed [`Effect`] algebra, and plain
//!   value-domain bounds on `I::Value`. Structured dialects additionally use
//!   [`Scope`]/[`ScopeHook`].
//! - **Compiler authors** compose languages into stage enums (deriving
//!   [`InterpDispatch`] alongside `StageMeta`) and run engines:
//!   [`ConcreteInterpreter`] for execution, [`AbstractInterpreter`] for
//!   lattice-based fixpoint analysis. Calling conventions are [`Linker`]
//!   components passed by value — [`SameStageLinker`] (default) or
//!   [`CrossStageLinker`] for multi-language pipelines.
//!
//! Engines interpret the same dialect rules: concrete and abstract execution
//! differ only in the value domain and in how undecided control flow
//! ([`Effect::Branch`], [`Effect::EnterAny`], [`ScopeStep::RepeatOrFinish`])
//! is driven.

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
pub use ctx::{Ctx, EnvOps, Interp};
pub use dispatch::{FunctionEntry, InterpDispatch, Interpretable};
pub use effect::{CallEffect, Callee, Edge, Effect, Scope, ScopeBody, ScopeHook, ScopeStep};
pub use env::{Env, EnvIndex, EnvStackStore};

pub use error::InterpreterError;
// The shared frame protocol (concrete + abstract implement it).
pub use frame::{AbstractFrameDriver, Frame, FrameDriver, FrameEffect};
// Concrete standard frames.
pub use concrete_frame::{CallFrame, Completion, FrameBuild, ScopeFrame, StandardFrame};
// Abstract standard frames.
pub use abstract_frame::{
    AbstractCallFrame, AbstractCfgFrame, AbstractCompletion, AbstractFrameBuild,
    AbstractFunctionFrame, AbstractScopeAlternativesFrame, AbstractScopeFrame,
    StandardAbstractFrame,
};
pub use linker::{CrossStageLinker, FunctionTarget, Linker, SameStageLinker};
pub use query::StageQuery;
pub use value::{BranchCondition, HasProductValue, expect_single};

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::{FunctionEntry, InterpDispatch, Interpretable};

/// Everything a dialect author needs to implement interpretation.
pub mod dialect {
    pub use crate::{
        BranchCondition, CallEffect, Callee, Ctx, Edge, Effect, EnvOps, FunctionEntry,
        HasProductValue, Interp, Interpretable, InterpreterError, Scope, ScopeHook, ScopeStep,
    };
}

/// Everything a compiler author needs to run interpreters and analyses.
///
/// Ordinary users need only the engines + [`Linker`] + value/error domain.
/// Advanced compiler/analysis authors customize *traversal* with a custom frame
/// type: concrete via `ConcreteInterpreter<.., F>` reusing [`ScopeFrame`]/
/// [`CallFrame`] through [`FrameBuild`]; abstract via `AbstractInterpreter<.., P, F>`
/// reusing the `Abstract*Frame`s through [`AbstractFrameBuild`]. Both implement
/// the shared [`Frame`]/[`FrameDriver`] protocol ([`AbstractFrameDriver`] adds the
/// abstract-only capabilities). Analysis *policy* is [`CallContext`] + [`WideningStrategy`].
pub mod engine {
    pub use crate::{
        AbstractCallFrame, AbstractCfgFrame, AbstractCompletion, AbstractFrameBuild,
        AbstractFrameDriver, AbstractFunctionFrame, AbstractInterpreter,
        AbstractScopeAlternativesFrame, AbstractScopeFrame, CallContext, CallFrame, Callee,
        Completion, ConcreteInterpreter, ContextInsensitive, CrossStageLinker, Frame, FrameBuild,
        FrameDriver, FrameEffect, FunctionTarget, InterpDispatch, InterpreterError, Linker,
        SameStageLinker, ScopeFrame, StandardAbstractFrame, StandardFrame, WideningStrategy,
        expect_single,
    };
}
