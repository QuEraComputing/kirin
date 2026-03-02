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
#[kirin(terminator, fn, type = T)]
pub enum ControlFlow<T: CompileTimeValue + Default> {
    #[chumsky(format = "br {target}({args})")]
    Branch {
        target: Successor,
        args: Vec<SSAValue>,
    },
    #[chumsky(
        format = "cond_br {condition} then={true_target}({true_args}) else={false_target}({false_args})"
    )]
    ConditionalBranch {
        condition: SSAValue,
        true_target: Successor,
        true_args: Vec<SSAValue>,
        false_target: Successor,
        false_args: Vec<SSAValue>,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
}

#[cfg(feature = "interpret")]
mod interpret_impl;
