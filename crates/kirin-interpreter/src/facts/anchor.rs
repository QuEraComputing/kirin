//! Locations in the IR that dataflow reasoning refers to: lattice anchors
//! (*where* facts attach), scope qualification, and change detection.
//!
//! Following MLIR's terminology, a lattice fact is attached to a *lattice
//! anchor*: sparse analyses anchor facts to [`SSAValue`]s; dense analyses
//! anchor facts to [`ProgramPoint`]s. Which anchor family an analysis uses is
//! part of its solver *shape*
//! ([`AnalysisShape`](crate::AnalysisShape) — see
//! [`semantics`](crate::semantics)); anchors themselves carry no dispatch
//! meaning. What a rule *means* is a separate concern entirely: the
//! [`SemanticKey`](crate::SemanticKey) dispatch tags live in
//! [`semantics`](crate::semantics), not here.
//!
//! The context a fact belongs to is its environment ([`EnvStore`](crate::EnvStore)),
//! not part of the anchor: the same anchor in two environments is two facts.

use std::hash::Hash;

use kirin_ir::{Block, SSAValue, Statement};

// ===========================================================================
// Lattice anchors
// ===========================================================================

/// A type that can anchor a lattice fact.
///
/// Anchors key fact stores ([`FactStore`](crate::FactStore)), so they must be
/// cheap to clone, compare, and hash. Sparse anchors are [`SSAValue`]s; dense
/// anchors are [`ProgramPoint`]s.
pub trait LatticeAnchor: Clone + Eq + Hash {}

impl LatticeAnchor for SSAValue {}

impl LatticeAnchor for Block {}

/// A location at which a dense dataflow fact is defined.
///
/// Blocks and statements each have two distinct boundary points. Whole CFG and
/// graph bodies are deliberately not points: unlike a block, they do not have
/// one unambiguous entry/exit fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgramPoint {
    /// State on entry to a CFG-owned or statement-owned block.
    BlockEntry(Block),
    /// State on exit from a CFG-owned or statement-owned block.
    BlockExit(Block),
    /// The point immediately before `stmt` executes.
    Before(Statement),
    /// The point immediately after `stmt` executes.
    After(Statement),
}

impl LatticeAnchor for ProgramPoint {}
