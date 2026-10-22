//! Matching and scheduling for Kirin rewrite rules.
//!
//! A rule is a `wildcard.pattern` from `kirin-wildcard`, usually written with
//! `#[rewrite_rule]`. A [`RuleSet`] checks the patterns once and holds the
//! [`NativeRegistry`] of Rust functions they call by name. A
//! [`RewriteDriver`] walks the IR in a [`WalkOrder`](kirin_ir::walk::WalkOrder),
//! matches every rule at every statement, runs the rewrite of each match
//! through the pass's [`Rewriter`](kirin_ir::Rewriter), and repeats until a
//! walk changes nothing.
//!
//! Concrete IR mutation stays in `kirin-ir`, and the rule data in
//! `kirin-wildcard`.
#![forbid(unsafe_code)]

mod driver;
mod native;
mod rule;

pub use driver::{DriverError, RewriteDriver, RewriteSummary};
pub use native::{ActionFn, CheckFn, NativeRegistry, NativeValue};
pub use rule::{RewriteRule, RuleError, RuleErrorKind, RuleSet};
