//! Backward dataflow analysis for Kirin IR — starting with **liveness**.
//!
//! # What this is (and is not)
//!
//! Liveness is *backward dataflow*: it solves flow equations over the IR's
//! use/def facts, it does **not** execute statements. It therefore lives
//! outside the forward interpreter framework ([`kirin_interpreter`]'s
//! `Interpretable`/`Ctx`/`Effect`) and is **not** part of the dialect-author
//! surface. As the workspace `CLAUDE.md` puts it: backward analyses belong in a
//! separate direction-parametric dataflow solver sharing the lattice traits;
//! they are not forced through `Interpretable`/`Effect`.
//!
//! This crate is *compiler-author* infrastructure — for passes such as dead
//! code elimination, register/memory planning, and diagnostics. It is public in
//! the crate sense, not a user-facing language API.
//!
//! # The analysis
//!
//! For every program point it computes the set of SSA values live *before* and
//! *after* each statement, plus each block's live-in/live-out. The transfer is
//!
//! ```text
//! live_before(stmt) = uses(stmt) ∪ (live_after(stmt) − defs(stmt))
//! uses(stmt) = stmt.arguments()      defs(stmt) = stmt.results()
//! join = set union                   bottom = ∅
//! ```
//!
//! with one refinement: a **pure** statement whose results are all dead does
//! not make its operands live (`live_before = live_after`). Purity comes from
//! the IR's [`IsPure`](kirin_ir::IsPure) fact.
//!
//! Most ops use this generic transfer over [`HasArguments`](kirin_ir::HasArguments)
//! / [`HasResults`](kirin_ir::HasResults) / purity facts. The two things the
//! generic traits cannot express — per-successor **edge arguments** and
//! **structured-control-flow shape** — come through the small [`LivenessOp`]
//! hook (see its docs). Dialect authors do not implement it; this crate
//! provides leaf impls for `kirin-cf`, `kirin-scf`, and `kirin-function::Return`,
//! and a compiler author writes one forwarding impl for their composed language.
//!
//! # Block-argument edge transfer
//!
//! CFG edges carry block arguments. Liveness maps a successor's *live block
//! parameters* back to the specific values the predecessor passes on that edge.
//! Given `^t(%a): … ; br ^t(%x)`, `%x` is live before the branch iff `%a` is
//! live at `^t`'s entry. Non-parameter values live at the target flow through
//! to the predecessor unchanged.
//!
//! # v1 scope
//!
//! Intraprocedural; supports CFG (`kirin-cf` `br`/`cond_br`), `kirin-function`
//! returns, plain/arith/cmp/constant ops, calls (treated as `uses = args`,
//! `defs = results` — no interprocedural propagation), and structured control
//! flow (`kirin-scf` `if`/`for`, the latter via a loop-carried fixpoint).
//! Unmodelled structured ops or malformed edges are explicit
//! [`LivenessError`]s rather than silent approximations.

mod error;
mod live_set;
mod op;
mod result;
mod solver;

pub use error::LivenessError;
pub use live_set::LiveSet;
pub use op::{Edge, Flow, LivenessOp};
pub use result::Liveness;
pub use solver::{analyze_function, analyze_liveness_by_name};

#[cfg(test)]
mod tests;
