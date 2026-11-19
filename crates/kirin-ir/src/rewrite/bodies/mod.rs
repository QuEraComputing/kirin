//! Bodies owned by a statement: creating them, adopting them for
//! [`Rewriter::create`](crate::Rewriter::create) and
//! [`Rewriter::replace_statement`](crate::Rewriter::replace_statement), and
//! erasing and tombstoning them.

mod adopt;
mod create;
mod erase;
mod nesting;

pub(super) use adopt::bodies_of;
