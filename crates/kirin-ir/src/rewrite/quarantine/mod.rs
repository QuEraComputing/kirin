//! The failure artifact of a rewrite pass.
//!
//! A pass that fails part-way through leaves edits already applied. The stage
//! is not rolled back. Instead, [`Quarantined`] takes ownership of it so that it
//! can still be inspected but can no longer be mistaken for usable IR.

mod cause;
mod quarantined;
mod report;

pub use cause::QuarantineCause;
pub use quarantined::Quarantined;
