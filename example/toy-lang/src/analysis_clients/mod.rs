//! Interpreter analyses as clients of `kirin-rewrite`'s analysis database:
//! stage-wide constant propagation, read by a test-only constant-folding rule,
//! and stage-wide demand, read by a test-only rule that frees undemanded
//! values for the driver's dead-code cleanup.

mod common;
mod constprop;
mod demand;
