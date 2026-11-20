//! Bodies owned by a statement: creating them, adopting them for inserts and
//! [`Rewriter::replace_statement`](crate::Rewriter::replace_statement), and
//! tombstoning them.

mod adopt;
mod create;
mod erase;
mod nesting;

pub(super) use adopt::bodies_of;
