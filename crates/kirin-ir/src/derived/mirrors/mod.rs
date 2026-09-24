//! The individual mirrors: one file each, holding that mirror's derivation,
//! installation, and comparison rules together.
//!
//! Adding a mirror means adding one file here and one field to
//! [`Mirrors`](super::Mirrors). The storage ([`SlotMap`](super::SlotMap)), the
//! comparison helper, and the bundle itself live in the parent module, since
//! they are shared by every mirror rather than owned by any one of them.

pub(super) mod block_body;
pub(super) mod predecessors;
pub(super) mod uses;
