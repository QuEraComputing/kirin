//! Unstructured control flow dialect for Kirin.
//!
//! This dialect provides low-level branching operations that correspond to
//! MLIR's `cf` dialect. It models unconditional branches (`br`) and
//! conditional branches (`cond_br`) with block arguments.
//!
//! # Operations
//!
//! | Operation | Description |
//! |-----------|-------------|
//! | `br ^target(args)` | Unconditional branch with block arguments |
//! | `cond_br %c then=^t(args) else=^f(args)` | Conditional branch |
//!
//! # Note on Return
//!
//! `Return` is intentionally absent from this dialect. Use
//! [`kirin_function::Return`] as the canonical return operation. This avoids
//! duplication between `kirin-cf` and `kirin-function`.
//!
//! # MLIR Correspondence
//!
//! - `Branch` ↔ `cf.br`
//! - `ConditionalBranch` ↔ `cf.cond_br`

use kirin::prelude::*;

#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[non_exhaustive]
#[kirin(terminator, builders, branch_op, type = T)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ControlFlow<T: CompileTimeValue> {
    #[chumsky(format = "$br {target}({args})")]
    Branch {
        target: Successor,
        args: Vec<SSAValue>,
    },
    #[chumsky(
        format = "$cond_br {condition} then={true_target}({true_args}) else={false_target}({false_args})"
    )]
    ConditionalBranch {
        condition: SSAValue,
        true_target: Successor,
        true_args: Vec<SSAValue>,
        false_target: Successor,
        false_args: Vec<SSAValue>,
    },
    #[doc(hidden)]
    __Phantom(std::marker::PhantomData<T>),
}

/// `br` forwards `args` to its only successor; `cond_br` forwards
/// `true_args` to successor 0 and `false_args` to successor 1. The condition
/// is not forwarded anywhere.
impl<T: CompileTimeValue> BranchOp for ControlFlow<T> {
    fn successor_operands(&self, index: usize) -> Option<&[SSAValue]> {
        match (self, index) {
            (ControlFlow::Branch { args, .. }, 0) => Some(args),
            (ControlFlow::ConditionalBranch { true_args, .. }, 0) => Some(true_args),
            (ControlFlow::ConditionalBranch { false_args, .. }, 1) => Some(false_args),
            _ => None,
        }
    }

    fn successor_operands_mut(&mut self, index: usize) -> Option<&mut Vec<SSAValue>> {
        match (self, index) {
            (ControlFlow::Branch { args, .. }, 0) => Some(args),
            (ControlFlow::ConditionalBranch { true_args, .. }, 0) => Some(true_args),
            (ControlFlow::ConditionalBranch { false_args, .. }, 1) => Some(false_args),
            _ => None,
        }
    }
}

pub mod interpreter;

#[cfg(test)]
mod tests;
