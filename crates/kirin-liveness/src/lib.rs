//! Classic SSA liveness for Kirin: a [`LiveSet`] at every program point (the
//! regalloc-grade fact), computed by [`DenseLiveness`], the dense backward
//! engine ([`kirin_interpreter::DenseBackwardInterpreter`]) at [`LiveSet`]
//! point states. Every use gens and every definition kills; purity is
//! irrelevant, so dead code's operands appear in these sets. Transfer comes
//! from each dialect's `Interpretable<I, ClassicLiveness>` rule.
//!
//! Strong liveness (which values are transitively needed by a root) is the
//! separate sparse backward analysis in `kirin-demand`.
//!
//! ```ignore
//! use kirin_interpreter::AnalysisResults;
//!
//! let analyzed = DenseLiveness::<Stage>::new(&pipeline).analyze(stage, callee)?;
//! let live = analyzed.fact(analyzed.root(), point);
//! ```

mod live;

pub use live::LiveSet;

use kirin_interpreter::{
    DenseBackwardInterpreter, DenseBlockFrame, InterpreterError, SameStageLinker,
};

/// The dense backward engine instantiated at [`LiveSet`] point states:
/// classic per-program-point liveness. A language with structured dialects
/// supplies its own private stack-item `F` embedding the dialect's dense frames.
pub type DenseLiveness<
    'ir,
    S,
    E = InterpreterError,
    F = DenseBlockFrame<LiveSet, E>,
    Lk = SameStageLinker,
> = DenseBackwardInterpreter<'ir, S, LiveSet, E, F, Lk>;
