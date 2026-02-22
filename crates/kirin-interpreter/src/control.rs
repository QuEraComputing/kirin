use std::convert::Infallible;

use kirin_ir::{Block, ResultValue, SpecializedFunction};
use smallvec::SmallVec;

/// Describes how execution continues after interpreting a statement.
///
/// The `Ext` parameter allows interpreter-specific variants. Defaults to
/// [`Infallible`] (no extra variants), which is sufficient for abstract
/// interpreters. Concrete interpreters use [`ConcreteExt`] for `Break`/`Halt`.
#[derive(Debug)]
pub enum Continuation<V, Ext = Infallible> {
    /// Advance to the next statement in the current block.
    Continue,
    /// Jump to a target block, binding argument values to its block arguments.
    Jump(Block, Vec<V>),
    /// Fork into multiple targets (undecidable branch).
    ///
    /// Lives in the base enum rather than as an `Ext` variant because
    /// dialect impls (e.g. `kirin-cf` conditional branches) are generic
    /// over `I: Interpreter` and need to construct `Fork` without knowing
    /// the concrete `Ext` type.
    ///
    /// Only reachable when [`crate::BranchCondition::is_truthy`] returns
    /// `None`, which cannot happen for concrete values. The concrete
    /// interpreter panics if it encounters this variant.
    Fork(SmallVec<[(Block, Vec<V>); 2]>),
    /// Call a specialized function with arguments, writing the return value
    /// to `result` in the caller's frame.
    Call {
        callee: SpecializedFunction,
        args: Vec<V>,
        /// Where to write the return value in the caller's frame.
        result: ResultValue,
    },
    /// Return a single value from the current function frame.
    Return(V),
    /// Interpreter-specific extension variant.
    Ext(Ext),
}

/// Extension variants for concrete (stack-based) interpretation.
#[derive(Debug)]
pub enum ConcreteExt {
    /// Suspend execution at the current statement (debugger breakpoint).
    Break,
    /// Terminate the session.
    Halt,
}

/// Continuation type for concrete interpreters.
pub type ConcreteContinuation<V> = Continuation<V, ConcreteExt>;

/// Continuation type for abstract interpreters (no extension variants).
pub type AbstractContinuation<V> = Continuation<V>;
