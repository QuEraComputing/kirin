//! Interpreter analyses as clients of `kirin-rewrite`'s analysis database:
//! stage-wide constant propagation, read by a constant-folding rule, and
//! stage-wide demand, read by a rule that frees undemanded values for the
//! driver's dead-code cleanup.

pub mod common;
pub mod constprop;
pub mod demand;
pub mod log;
