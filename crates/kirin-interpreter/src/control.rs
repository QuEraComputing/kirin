use std::convert::Infallible;

use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};
use smallvec::SmallVec;

use crate::{InterpreterError, ValueStore};

/// Inline argument list for continuation variants.
///
/// Most block/call arguments fit in 2 elements (or are empty), so we
/// avoid heap allocation for the common case.
pub type Args<V> = SmallVec<[V; 2]>;

/// Describes how execution continues after interpreting a statement.
///
/// The `Ext` parameter allows interpreter-specific variants. Defaults to
/// [`Infallible`] (no extra variants), which is sufficient for abstract
/// interpreters. Concrete interpreters use [`ConcreteExt`] for `Break`/`Halt`.
#[derive(Debug)]
#[must_use = "continuations must be handled to advance interpreter state"]
pub enum Continuation<V, Ext = Infallible> {
    /// Advance to the next statement in the current block.
    Continue,
    /// Jump to a target block, binding argument values to its block arguments.
    Jump(Block, Args<V>),
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
    Fork(SmallVec<[(Block, Args<V>); 2]>),
    /// Call a specialized function with arguments, writing the return values
    /// to `results` in the caller's frame.
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Args<V>,
        /// Where to write the return values in the caller's frame.
        results: SmallVec<[ResultValue; 1]>,
    },
    /// Return values from the current function frame.
    Return(SmallVec<[V; 1]>),
    /// Yield values from an inline body block (e.g. `scf.yield`) without
    /// popping a call frame. The parent operation handles cursor restoration.
    Yield(SmallVec<[V; 1]>),
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

/// Write multiple return/yield values to their corresponding result slots
/// with arity checking.
///
/// Returns [`InterpreterError::ArityMismatch`] if the number of values
/// does not match the number of result slots.
pub fn write_results<S>(
    store: &mut S,
    results: &[ResultValue],
    values: &SmallVec<[S::Value; 1]>,
) -> Result<(), S::Error>
where
    S: ValueStore,
    S::Error: From<InterpreterError>,
{
    if results.len() != values.len() {
        return Err(InterpreterError::ArityMismatch {
            expected: results.len(),
            got: values.len(),
        }
        .into());
    }
    for (rv, val) in results.iter().zip(values.iter()) {
        store.write(*rv, val.clone())?;
    }
    Ok(())
}
