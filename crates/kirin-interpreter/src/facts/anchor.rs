//! Dataflow vocabulary: analysis kinds, lattice anchors, scope qualification,
//! and change detection.
//!
//! Kirin models dataflow on two orthogonal axes — *lattice-anchor style*
//! (sparse vs dense) and *direction* (forward vs backward) — giving four
//! [analysis kinds](AnalysisKind). Following MLIR's terminology, a lattice fact
//! is attached to a *lattice anchor*: sparse analyses anchor facts to
//! [`SSAValue`]s, dense analyses anchor facts to blocks, edges, or program
//! points. The kind markers double as the [`Interp::Kind`](crate::Interp::Kind)
//! dialect-dispatch tags: `SparseForward` is the operational forward-evaluation
//! path (concrete execution, constprop, interval), `SparseBackward` is the
//! use-def-structured backward demand path (strong liveness), and
//! `DenseBackward` is the program-point backward path (classic liveness).
//!
//! Framework-level fact keys are never bare anchors: [`Scoped`] qualifies an
//! anchor with the scope/context it belongs to, so the same anchor under two
//! contexts is two facts.

use std::hash::Hash;

use kirin_ir::{Block, SSAValue, Statement};

// ===========================================================================
// Analysis kinds (lattice-anchor style × direction)
// ===========================================================================

/// Marker trait for the four dataflow analysis kinds.
///
/// The kind selects which lattice-anchor vocabulary and store are valid for an
/// analysis. The same markers double as the [`Interp::Kind`](crate::Interp::Kind)
/// dialect-dispatch tag, so one dialect type carries one
/// [`Interpretable`](crate::Interpretable) rule per kind without coherence
/// conflicts.
pub trait AnalysisKind {}

/// Facts attach to SSA values and propagate forward through def-use chains.
/// Concrete execution, constprop, and interval analysis are `SparseForward`.
pub struct SparseForward;

/// Facts attach to SSA values and propagate backward through use-def structure.
/// Strong (true) liveness, neededness, and demand are `SparseBackward`.
pub struct SparseBackward;

/// Facts attach to block/edge/program-point anchors and propagate forward.
/// Typestate and initialization are usually `DenseForward`.
pub struct DenseForward;

/// Facts attach to block/edge/program-point anchors and propagate backward.
/// Classic per-point liveness is `DenseBackward`.
pub struct DenseBackward;

impl AnalysisKind for SparseForward {}
impl AnalysisKind for SparseBackward {}
impl AnalysisKind for DenseForward {}
impl AnalysisKind for DenseBackward {}

// ===========================================================================
// Lattice anchors
// ===========================================================================

/// A type that can anchor a lattice fact.
///
/// Anchors key fact stores ([`FactStore`](crate::FactStore)) and summaries, so
/// they must be cheap to clone, compare, and hash. Sparse anchors are
/// [`SSAValue`]s; dense anchors are [`Block`]s, [`ProgramPoint`]s, or
/// [`DenseAnchor`]s; [`Scoped`] qualifies any anchor with its scope.
pub trait LatticeAnchor: Clone + Eq + Hash {}

impl LatticeAnchor for SSAValue {}

impl LatticeAnchor for Block {}

/// A program point: immediately before or after a statement.
///
/// Never anchor a fact to a raw statement without saying *before* or *after* —
/// the two carry different facts for any non-trivial analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgramPoint {
    /// The point immediately before `stmt` executes.
    Before(Statement),
    /// The point immediately after `stmt` executes.
    After(Statement),
}

impl LatticeAnchor for ProgramPoint {}

/// A dense lattice anchor: a block boundary, program point, or CFG edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DenseAnchor {
    /// State on entry to a block.
    BlockEntry(Block),
    /// State on exit from a block.
    BlockExit(Block),
    /// State at a specific [`ProgramPoint`].
    Point(ProgramPoint),
    /// State on a specific CFG edge.
    Edge { from: Block, to: Block },
}

impl LatticeAnchor for DenseAnchor {}

// ===========================================================================
// Scope qualification
// ===========================================================================

/// An anchor or owner qualified by the scope/context it belongs to.
///
/// Framework-level summary keys are never bare anchors: the same [`SSAValue`]
/// or [`Block`] under two scopes (two stages, two analyzed regions, two call
/// contexts) is two distinct facts, so keys carry their scope. Region-level
/// analyses use `(CompileStage, Region)` as the scope; interprocedural
/// analyses generalize `K` to a call-context key (the backward analogue of the
/// forward engine's context-qualified value keys).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scoped<K, T> {
    pub scope: K,
    pub item: T,
}

impl<K, T> Scoped<K, T> {
    pub fn new(scope: K, item: T) -> Self {
        Self { scope, item }
    }
}

impl<K, T> LatticeAnchor for Scoped<K, T>
where
    K: Clone + Eq + Hash,
    T: Clone + Eq + Hash,
{
}

// ===========================================================================
// Change detection
// ===========================================================================

/// Whether a store update changed anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The store was not modified.
    Unchanged,
    /// The store was modified; the fixpoint has not yet stabilized.
    Changed,
}

impl Change {
    /// `true` if this is [`Change::Changed`].
    pub fn changed(self) -> bool {
        matches!(self, Change::Changed)
    }

    /// Combine two change results: `Changed` if either changed.
    pub fn or(self, other: Change) -> Change {
        match (self, other) {
            (Change::Unchanged, Change::Unchanged) => Change::Unchanged,
            _ => Change::Changed,
        }
    }

    /// Fold a `bool` "did something change" into a [`Change`].
    pub fn from_bool(changed: bool) -> Change {
        if changed {
            Change::Changed
        } else {
            Change::Unchanged
        }
    }
}
