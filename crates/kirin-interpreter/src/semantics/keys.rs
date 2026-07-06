//! Semantic keys: the compile-time names of IR interpretations.
//!
//! Each shipped key is a zero-sized marker implementing [`SemanticKey`]. A key
//! declares the [`shape`](super::shape) its solver runs on and joins that
//! shape's family, which is what the generic engines bound their `Sem`
//! parameter with.

use super::shape::{
    AnalysisShape, DenseBackwardSemantic, DenseBackwardShape, SparseBackwardSemantic,
    SparseBackwardShape, SparseForwardSemantic, SparseForwardShape,
};

/// A semantic key: the compile-time name of one interpretation of the IR.
///
/// This is the dispatch tag of [`Interpretable<I, Semantics>`](crate::Interpretable):
/// one dialect type carries one rule per key without coherence conflicts, and
/// downstream crates add keys freely. Every key declares the [`AnalysisShape`]
/// its solver runs on.
pub trait SemanticKey {
    /// The solver shape this semantics runs on.
    type Shape: AnalysisShape;
}

/// Forward evaluation: run statements over a value domain. Concrete execution
/// and forward abstract interpretation (constant propagation, intervals) are
/// all `ForwardEval` — the value domain, not the key, distinguishes them.
pub struct ForwardEval;

impl SemanticKey for ForwardEval {
    type Shape = SparseForwardShape;
}

impl SparseForwardSemantic for ForwardEval {}

/// Strong (true) liveness as per-SSA-value demand: a value is demanded iff it
/// transitively feeds a root (return/terminator operand, impure statement).
pub struct StrongDemand;

impl SemanticKey for StrongDemand {
    type Shape = SparseBackwardShape;
}

impl SparseBackwardSemantic for StrongDemand {}

/// Classic per-program-point liveness: the textbook kill-defs/gen-all-uses
/// transfer over per-point live sets (the regalloc-grade fact).
pub struct ClassicLiveness;

impl SemanticKey for ClassicLiveness {
    type Shape = DenseBackwardShape;
}

impl DenseBackwardSemantic for ClassicLiveness {}
