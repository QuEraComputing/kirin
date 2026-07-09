//! Analysis shapes: the four solver-mechanics markers (anchoring × direction)
//! and the per-shape semantic families the generic engines bound on.

use super::keys::SemanticKey;

/// Marker trait for the four solver shapes (anchoring × direction).
///
/// A shape fixes mechanics: which [`LatticeAnchor`](crate::LatticeAnchor)
/// family facts attach to, which direction they propagate, and what the
/// engines' store/fixpoint expectations are. Shapes are never dispatch tags —
/// dialect rules dispatch on a [`SemanticKey`].
pub trait AnalysisShape {}

/// Facts attach to SSA values and propagate forward through def-use chains.
pub struct SparseForwardShape;

/// Facts attach to SSA values and propagate backward through use-def
/// structure.
pub struct SparseBackwardShape;

/// Facts attach to block/edge/program-point anchors and propagate forward.
pub struct DenseForwardShape;

/// Facts attach to block/edge/program-point anchors and propagate backward.
pub struct DenseBackwardShape;

impl AnalysisShape for SparseForwardShape {}
impl AnalysisShape for SparseBackwardShape {}
impl AnalysisShape for DenseForwardShape {}
impl AnalysisShape for DenseBackwardShape {}

// ===========================================================================
// Semantic families (one marker per shape)
// ===========================================================================
//
// A *family* groups the keys that run on one shape, and is what the generic
// engines bound their semantics parameter with: `SparseForwardTransfer<...,
// Sem>` requires `Sem: SparseForwardSemantic`, so instantiating an engine at a
// key of the wrong shape is a compile error. A downstream key joins a family
// with one extra line:
//
// ```ignore
// struct QubitAddress;
// impl SemanticKey for QubitAddress {
//     type Shape = SparseForwardShape;
// }
// impl SparseForwardSemantic for QubitAddress {}
// ```

/// Keys running on [`SparseForwardShape`].
pub trait SparseForwardSemantic: SemanticKey<Shape = SparseForwardShape> {}

/// Keys running on [`SparseBackwardShape`].
pub trait SparseBackwardSemantic: SemanticKey<Shape = SparseBackwardShape> {}

/// Keys running on [`DenseForwardShape`].
pub trait DenseForwardSemantic: SemanticKey<Shape = DenseForwardShape> {}

/// Keys running on [`DenseBackwardShape`].
pub trait DenseBackwardSemantic: SemanticKey<Shape = DenseBackwardShape> {}
