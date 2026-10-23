//! `DiGraph` body surgery for the [`Rewriter`](super::Rewriter).
//!
//! A `DiGraph`'s petgraph mirrors its members' operands: one edge
//! `producer → consumer` per operand slot whose value is a result of another
//! member. The rewriter keeps it (and the statement → node map beside it)
//! correct incrementally; there is no public edge API. Members are added with
//! [`Rewriter::insert_node`](super::Rewriter::insert_node) and erased through
//! [`Rewriter::erase_statement`](super::Rewriter::erase_statement); yields are
//! edited with [`Rewriter::replace_yield`](super::Rewriter::replace_yield).

mod surgery;
mod topology;
