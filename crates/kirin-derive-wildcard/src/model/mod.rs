//! What a `#[rewrite_rule]` function is lowered into before code generation.
//!
//! The model records what the function body says, in source terms:
//!  - values are referred to by [`ValueId`],
//!  - ops and native functions stay as the paths written in the source,
//!  - spans are kept for error messages, including the span of each [`Use`]
//!    of a value.
//!
//! [`RuleModel::verify`] checks that every matched op feeds the root and
//! that the rewrite's body uses nothing an earlier action of it removed.
//!
//! Its statements are already shaped like the pattern IR's, one per IR
//! statement. This means one source Rust statement can become several:
//! a constraint call gets decomposed into a [`StmtKind::NativeCompute`]
//! and [`StmtKind::Constraint`] on its result, and any hole in matching ops
//! becomes a [`StmtKind::Hole`] whose result is the hole's value.

mod rule;
mod statement;
mod verify;

pub(crate) use rule::RuleModel;
pub(crate) use statement::{Binding, StmtId, StmtKind, Use, ValueId};
