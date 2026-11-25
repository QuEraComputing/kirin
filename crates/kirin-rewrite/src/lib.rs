//! Matching and scheduling for Kirin rewrite rules.
//!
//! A pattern rule is a `wildcard.pattern` from `kirin-wildcard`, usually
//! written with `#[rewrite_rule]`. A [`RuleSet`] checks each
//! [`PatternRule`], a pattern with the Rust functions its calls run, once. A
//! [`RewriteDriver`] visits the statements of the IR from a worklist, filled
//! in a [`WalkOrder`](kirin_ir::walk::WalkOrder), matches every rule at each,
//! runs the rewrite of each match through the pass's
//! [`Rewriter`](kirin_ir::Rewriter), adds back the statements the edits may
//! give a new match, and repeats from every statement until a sweep changes
//! nothing.
//!
//! A rule may also be written in Rust: a [`StatementRule`] is handed each
//! statement the driver visits, and a [`BlockRule`] each block a visited
//! statement owns, for work a pattern can't express, such as
//! common-subexpression elimination. The rule set holds a recipe for each,
//! and the driver builds a fresh copy every sweep for a statement rule and
//! every block for a block rule. Rules of all kinds share one list and are
//! tried in its order.
//!
//! Concrete IR mutation stays in `kirin-ir`, and the rule data in
//! `kirin-wildcard`.
#![forbid(unsafe_code)]

mod driver;
mod rule;

pub use driver::{DriverError, RewriteDriver, RewriteSummary};
pub use kirin_wildcard::{ActionFn, CheckFn, NativeError, NativeFailure, NativeValue, PatternRule};
pub use rule::{BlockRule, CheckedRule, RuleError, RuleErrorKind, RuleSet, StatementRule};
