//! Derived metadata: derive, install, verify — never repair.
//!
//! A *mirror* is metadata that is fully recoverable from authoritative IR.
//! This is implemented in [`StageInfo`](crate::StageInfo) where the mirrors are
//! stored in the [`StageInfo::ssas`](crate::StageInfo::ssas) arenas, and the
//! authoritative IR is stored in the [`StageInfo::nodes`](crate::StageInfo::nodes)
//! arenas.
//!
//! Currently, there are four mirrors:
//!  1. [`SSAInfo::uses`](crate::SSAInfo) collects each SSA-value's uses. These
//!     mirror an SSA-value's uses in the IR's statement operand slots and `DiGraph` yield slots;
//!  2. [`BlockInfo::predecessors`](crate::BlockInfo) collects a block's predecessors.
//!     These mirror the successor references carried by terminators in the IR;
//!  3. [`BlockInfo::statements`](crate::BlockInfo) and
//!     [`BlockInfo::terminator`](crate::BlockInfo) summarize a block's body. These
//!     mirror the `prev`/`next` links on the block's member statements, partitioned
//!     into the non-terminator chain and the terminator that sits outside it;
//!  4. [`CFGInfo::blocks`](crate::node::CFGInfo) summarizes a CFG's block list. This mirrors the
//!     `prev`/`next` links on the blocks parented to that CFG.
//!
//! The last two are reconstructed by the shared chain walker [`derive_chains`](self::chain::derive_chains)
//!  in [`chain`](self::chain), which never reads the summary it is deriving.
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

mod chain;
mod compare;
mod error;
mod mirrors;

pub use chain::{ChainDefect, ChainFinding};
pub use error::{DanglingParent, DeriveError, Finding, Mismatch, VerifyError};
pub use mirrors::block_body::BlockBody;

use std::marker::PhantomData;

use crate::arena::{Arena, Id, Identifier};
use crate::derived::mirrors::block_body::{self, BlockBodyMap};
use crate::derived::mirrors::cfg_blocks::{self, CFGBlocksMap};
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
    /// An empty map with `len` entries, indexed by raw arena slot.
    fn with_len(len: usize) -> Self {
        Self {
            slots: vec![T::default(); len],
            marker: PhantomData,
        }
    }

    /// An empty map with one entry per slot of `source`, tombstones included,
    /// so an id indexes both the source Arena and this SlotMap identically.
    fn sized_like<U>(source: &Arena<I, U>) -> Self {
        Self::with_len(source.len())
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

    /// Iterate `(id, &value)` over every slot, tombstones included.
    ///
    /// [`SlotMap::into_iter`] drops the id, but a caller that walks containers
    /// needs to know which one each entry belongs to.
    fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.slots
            .iter()
            .enumerate()
            .map(|(raw, value)| (I::from(Id(raw)), value))
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
    block_bodies: BlockBodyMap,
    cfg_blocks: CFGBlocksMap,
}

fn derive_partial<L: Dialect>(stage: &StageInfo<L>) -> (Mirrors, Vec<Finding>) {
    let mut findings = Vec::new();

    let uses = uses::derive(stage, &mut findings);
    let predecessors = predecessors::derive(stage, &mut findings);
    let block_bodies = block_body::derive(stage, &mut findings);
    let cfg_blocks = cfg_blocks::derive(stage, &mut findings);

    (
        Mirrors {
            uses,
            predecessors,
            block_bodies,
            cfg_blocks,
        },
        findings,
    )
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
pub(crate) fn install_mirrors<L: Dialect>(stage: &mut StageInfo<L>, mirrors: Mirrors) {
    let Mirrors {
        uses,
        predecessors,
        block_bodies,
        cfg_blocks,
    } = mirrors;
    uses.install(stage);
    predecessors.install(stage);
    block_bodies.install(stage);
    cfg_blocks.install(stage);
}

/// Best-effort: for use by `finalize_unchecked`.
/// Installs every entry that could be derived and discards the findings;
/// If the IR is then genuinely broken, `verify_derived` will say so.
pub(crate) fn install_and_derive_mirrors_unchecked<L: Dialect>(stage: &mut StageInfo<L>) {
    let (mirrors, _findings) = derive_partial(stage);
    install_mirrors(stage, mirrors);
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
    mirrors.block_bodies.verify(stage, &mut mismatches);
    mirrors.cfg_blocks.verify(stage, &mut mismatches);

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(VerifyError::Mismatch(mismatches))
    }
}
