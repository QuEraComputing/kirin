//! `UnGraph` body surgery for the [`Rewriter`](super::Rewriter).
//!
//! An `UnGraph`'s petgraph mirrors its wires: an edge value (an edge
//! statement's result or a boundary port) read by exactly two node operand
//! slots is one undirected edge between those nodes. Valid IR has at most
//! two such endpoints per wire, but the rewriter allows more mid-pass: such a
//! wire has no edge, and [`verify_derived`](crate::verify_derived) reports it.
//! The rewriter keeps the petgraph (and the member map beside it)
//! correct incrementally; there is no public edge API.

mod surgery;
mod topology;
