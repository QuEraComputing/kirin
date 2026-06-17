//! The compiler-facing classification hook for liveness.
//!
//! Backward liveness needs two facts that the generic IR traits cannot supply:
//!
//! * **Per-successor edge arguments.** `HasSuccessors` yields target blocks but
//!   not the argument list passed on each edge, and a conditional branch
//!   interleaves both successors' args in `arguments()`. Liveness must map a
//!   successor's *live block parameters* back to the *specific* values the
//!   predecessor passes on that edge.
//! * **Structured-control-flow shape.** `scf.if`/`scf.for` need their condition,
//!   body blocks, loop-carried partitioning and result slots — none of which is
//!   recoverable from `arguments()`/`results()` alone.
//!
//! So a dialect op classifies itself into a [`Flow`]. This is a *compiler-author*
//! hook, not part of the dialect-author surface: dialect authors never implement
//! it. This crate provides the leaf impls for the standard control-flow dialects
//! (`kirin-cf`, `kirin-scf`, `kirin-function::Return`); a compiler author writes
//! one small forwarding impl for their composed language enum that delegates to
//! the wrapped op (a `#[derive(LivenessOp)]` that auto-forwards `#[wraps]`
//! variants is possible future work).

use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_ir::{Block, CompileTimeValue, SSAValue};
use kirin_scf::{For, If, StructuredControlFlow, Yield};

/// A single CFG successor edge: a target block and the values passed to its
/// block parameters, positionally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// The successor block.
    pub target: Block,
    /// Values supplied to `target`'s block parameters, in order.
    pub args: Vec<SSAValue>,
}

/// How an op participates in backward liveness.
///
/// Operands that are *direct uses* of a control op (e.g. a conditional
/// branch's condition) are recovered by the solver from `arguments()` minus
/// the edge args, so they are deliberately absent from [`Flow::Branch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    /// An ordinary op: the generic `uses ∪ (live_after − defs)` transfer
    /// applies (with the dead-pure-op refinement).
    Plain,
    /// A CFG branch terminator with one edge per successor.
    Branch(Vec<Edge>),
    /// A return-like terminator: no successors; its operands are uses.
    Return,
    /// A structured two-way conditional (`scf.if`). Each arm is a single block
    /// terminated by `yield`; `results` are positionally matched to each arm's
    /// yielded values.
    If {
        /// The branch condition (a direct use).
        condition: SSAValue,
        /// The `then` arm body block.
        then_block: Block,
        /// The `else` arm body block.
        else_block: Block,
        /// Result slots, matched to each arm's `yield` values.
        results: Vec<SSAValue>,
    },
    /// A structured counted loop (`scf.for`). The body is a single block whose
    /// first parameter is the induction variable and whose remaining
    /// parameters are loop-carried, positionally matched to `init_args`, the
    /// body's `yield` values, and `results`.
    For {
        /// Lower bound of the induction range (a direct use).
        start: SSAValue,
        /// Upper bound (exclusive) of the induction range (a direct use).
        end: SSAValue,
        /// Induction step (a direct use).
        step: SSAValue,
        /// Initial loop-carried values.
        init_args: Vec<SSAValue>,
        /// The loop body block.
        body: Block,
        /// Result slots (final loop-carried values).
        results: Vec<SSAValue>,
    },
    /// A structured `yield` terminator inside an `scf` body.
    Yield {
        /// Yielded values, matched to the parent's result slots.
        values: Vec<SSAValue>,
    },
}

/// Classifies an op for backward liveness. See the [module docs](self).
pub trait LivenessOp {
    /// Classify this op into a [`Flow`].
    fn flow(&self) -> Flow;
}

impl<T: CompileTimeValue> LivenessOp for ControlFlow<T> {
    fn flow(&self) -> Flow {
        match self {
            ControlFlow::Branch { target, args } => Flow::Branch(vec![Edge {
                target: target.target(),
                args: args.clone(),
            }]),
            ControlFlow::ConditionalBranch {
                condition: _,
                true_target,
                true_args,
                false_target,
                false_args,
            } => Flow::Branch(vec![
                Edge {
                    target: true_target.target(),
                    args: true_args.clone(),
                },
                Edge {
                    target: false_target.target(),
                    args: false_args.clone(),
                },
            ]),
            // `#[non_exhaustive]` phantom variant.
            _ => Flow::Plain,
        }
    }
}

impl<T: CompileTimeValue> LivenessOp for Return<T> {
    fn flow(&self) -> Flow {
        Flow::Return
    }
}

fn results_as_ssa(results: &[kirin_ir::ResultValue]) -> Vec<SSAValue> {
    results.iter().copied().map(SSAValue::from).collect()
}

impl<T: CompileTimeValue> LivenessOp for If<T> {
    fn flow(&self) -> Flow {
        Flow::If {
            condition: self.condition(),
            then_block: self.then_block(),
            else_block: self.else_block(),
            results: results_as_ssa(self.results()),
        }
    }
}

impl<T: CompileTimeValue> LivenessOp for For<T> {
    fn flow(&self) -> Flow {
        Flow::For {
            start: self.start(),
            end: self.end(),
            step: self.step(),
            init_args: self.init_args().to_vec(),
            body: self.body(),
            results: results_as_ssa(self.results()),
        }
    }
}

impl<T: CompileTimeValue> LivenessOp for Yield<T> {
    fn flow(&self) -> Flow {
        Flow::Yield {
            values: self.values().to_vec(),
        }
    }
}

impl<T: CompileTimeValue> LivenessOp for StructuredControlFlow<T> {
    fn flow(&self) -> Flow {
        match self {
            StructuredControlFlow::If(op) => op.flow(),
            StructuredControlFlow::For(op) => op.flow(),
            StructuredControlFlow::Yield(op) => op.flow(),
        }
    }
}
