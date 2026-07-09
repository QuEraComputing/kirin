//! Semantic keys (what a dialect rule *means*) vs analysis shapes (how a
//! solver *runs*).
//!
//! These are two different vocabularies, deliberately kept apart:
//!
//! - An [`AnalysisShape`] describes solver mechanics only: sparse vs dense
//!   anchoring, forward vs backward propagation — and with them the expected
//!   anchor family, store, and fixpoint discipline. Shapes are a closed set of
//!   four axes-combinations and say nothing about what the facts mean.
//! - A [`SemanticKey`] names an *interpretation* of the IR: forward
//!   evaluation, strong demand, classic liveness, … It is the compile-time
//!   dispatch tag of [`Interpretable<I, Semantics>`](crate::Interpretable) and
//!   [`Interp::Semantics`](crate::Interp::Semantics) — the thing a dialect
//!   author writes rules *for*. Every key declares the shape its solver runs
//!   on ([`SemanticKey::Shape`]), so choosing a semantics picks the mechanics,
//!   but dialect rules never dispatch on a raw solver axis.
//!
//! Keeping them separate is what lets two semantics share one shape without
//! colliding: [`ClassicLiveness`] runs on [`DenseBackwardShape`], and a future
//! dense backward analysis (e.g. typestate-style very-busy expressions) adds
//! its own key on the same shape — same engines, distinct `Interpretable`
//! rules, no coherence conflict.
//!
//! # Kirin 1.0 semantic keys
//!
//! Kirin 1.0 dispatched interpretations by *string* keys — `"main"` (concrete
//! evaluation), `"typeinfer"`, `"constprop"`, `"qubit.address"` — looked up at
//! runtime. The Rust model replaces each string with a zero-sized marker type
//! implementing [`SemanticKey`]: [`ForwardEval`] plays `"main"`'s role (and
//! `"constprop"`'s — the value domain, not the key, distinguishes them),
//! [`StrongDemand`]/[`ClassicLiveness`] are the liveness keys, and a
//! downstream crate adds e.g. `"qubit.address"` as
//!
//! ```ignore
//! struct QubitAddress;
//! impl SemanticKey for QubitAddress {
//!     type Shape = SparseForwardShape;
//! }
//! ```
//!
//! gaining dispatch (`Interpretable<I, QubitAddress>`) without touching the
//! framework — the compile-time analogue of registering a new key string.

pub(crate) mod keys;
pub(crate) mod shape;

pub use keys::{ClassicLiveness, ForwardEval, SemanticKey, StrongDemand};
pub use shape::{
    AnalysisShape, DenseBackwardSemantic, DenseBackwardShape, DenseForwardSemantic,
    DenseForwardShape, SparseBackwardSemantic, SparseBackwardShape, SparseForwardSemantic,
    SparseForwardShape,
};
