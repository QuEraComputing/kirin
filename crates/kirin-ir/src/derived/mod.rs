//! Derived metadata: derive, install, verify — never repair.
//!
//! A *mirror* is metadata that is fully recoverable from authoritative IR.
//! This is implemented in [`StageInfo`](crate::StageInfo) where the mirrors are
//! stored in the [`StageInfo::ssas`](crate::StageInfo::ssas) arenas, and the
//! authoritative IR is stored in the [`StageInfo::nodes`](crate::StageInfo::nodes)
//! arenas.
//!
//! Currently, there are two mirrors:
//!  1. [`SSAInfo::uses`](crate::SSAInfo) collects each SSA-value's uses. These
//!     mirror an SSA-value's uses in the IR's statement operand slots and `DiGraph` yield slots;
//!  2. [`BlockInfo::predecessors`](crate::BlockInfo) collects a block's predecessors.
//!     These mirror the successor references carried by terminators in the IR.
//!
//! The authoritative slots in the IR are the ground truth; a mirror is a
//! cache over them that exists to answer the reverse question cheaply.
//!
//! All derived services follow a split:
//!
//! - [`derive_mirrors`] is **pure**: it reads a stage and computes what the
//!   mirrors *should* be, reporting [`Finding`]s when the authoritative IR is
//!   itself broken.
//! - [`install_mirrors`] writes them, and is used **only** at finalization,
//!   before a stage is published.
//! - [`verify_derived`] is **read-only**: it derives a fresh copy, compares it
//!   with what is installed, and throws the copy away.
//!
//! The two error types name the two different diagnoses:
//!
//! - [`DeriveError`] — the authoritative IR is corrupt (an operand names a
//!   tombstoned value, a terminator targets a deleted block).
//! - [`VerifyError::Mismatch`] — the authoritative IR is fine, but installed
//!   metadata disagrees with it, so the **mutation layer** is at fault.

mod compare;
mod error;
mod mirrors;

pub use error::{DeriveError, Finding, Mismatch, VerifyError};

use std::marker::PhantomData;

use crate::arena::{Arena, Id, Identifier};
use crate::{Dialect, StageInfo};

use self::mirrors::predecessors::{self, PredecessorMap};
use self::mirrors::uses::{self, UseMap};
/// A map from an arena id to a value.
///
/// Used to collect the mirrors [`SSAInfo::uses`](crate::SSAInfo) and
/// [`BlockInfo::predecessors`](crate::BlockInfo) in one place.
/// The `I` parameter ensures through the type system that a `Block` cannot be
/// used to index an `SSAValue` map, and vice versa.
struct SlotMap<I: Identifier, T> {
    slots: Vec<T>,
    marker: PhantomData<I>,
}

impl<I: Identifier, T: Clone + Default> SlotMap<I, T> {
    /// An empty map with one entry per slot of `source`, tombstones included,
    /// so an id indexes both the source Arena and this SlotMap identically.
    fn sized_like<U>(source: &Arena<I, U>) -> Self {
        Self {
            slots: vec![T::default(); source.len()],
            marker: PhantomData,
        }
    }
}

impl<I: Identifier, T> SlotMap<I, T> {
    fn get(&self, id: I) -> Option<&T> {
        let slot: Id = id.into();
        self.slots.get(slot.raw())
    }

    fn get_mut(&mut self, id: I) -> Option<&mut T> {
        let slot: Id = id.into();
        self.slots.get_mut(slot.raw())
    }

    fn into_iter(self) -> std::vec::IntoIter<T> {
        self.slots.into_iter()
    }

    fn len(&self) -> usize {
        self.slots.len()
    }
}

/// Every derived mirror of one stage, collected by [`derive_mirrors`] in its
/// traversal of the IR. The mirrors' implementations live in [`mirrors`]; this is the
/// bundle they travel in together.
pub(crate) struct Mirrors {
    uses: UseMap,
    predecessors: PredecessorMap,
}

fn derive_partial<L: Dialect>(stage: &StageInfo<L>) -> (Mirrors, Vec<Finding>) {
    let mut findings = Vec::new();

    let uses = uses::derive(stage, &mut findings);
    let predecessors = predecessors::derive(stage, &mut findings);

    (Mirrors { uses, predecessors }, findings)
}

/// Compute what every mirror of `stage` should be. Pure: `stage` is not written.
///
/// Component derivations share one findings buffer instead of returning early,
/// so a single scan reports every independent defect rather than stopping at
/// the first one.
pub(crate) fn derive_mirrors<L: Dialect>(stage: &StageInfo<L>) -> Result<Mirrors, DeriveError> {
    let (mirrors, findings) = derive_partial(stage);

    if findings.is_empty() {
        Ok(mirrors)
    } else {
        Err(DeriveError::new(findings))
    }
}

/// Write derived mirrors into a stage.
#[allow(dead_code, clippy::unused_self)]
pub(crate) fn install_mirrors<L: Dialect>(stage: &mut StageInfo<L>, mirrors: Mirrors) {
    let Mirrors { uses, predecessors } = mirrors;
    uses.install(stage);
    predecessors.install(stage);
}

/// Check that the mirrors installed in `stage` match a fresh derivation.
/// Read-only: the derived copy is compared and dropped.
///
/// # Errors
///
/// - [`VerifyError::Derive`] — the authoritative IR is corrupt, so no
///   comparison was possible. Investigate the IR.
/// - [`VerifyError::Mismatch`] — the IR is sound but installed metadata is
///   stale. Investigate the mutation path that produced it.
pub fn verify_derived<L: Dialect>(stage: &StageInfo<L>) -> Result<(), VerifyError> {
    let mirrors = derive_mirrors(stage)?;

    let mut mismatches: Vec<Mismatch> = Vec::new();
    mirrors.uses.verify(stage, &mut mismatches);
    mirrors.predecessors.verify(stage, &mut mismatches);

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(VerifyError::Mismatch(mismatches))
    }
}
