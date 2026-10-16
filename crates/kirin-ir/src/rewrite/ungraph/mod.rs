//! `UnGraph` body surgery for the [`Rewriter`](super::Rewriter).
//!
//! An `UnGraph`'s petgraph mirrors its wires: an edge value (an edge
//! statement's result or a boundary port) read by exactly two node operand
//! slots is one undirected edge between those nodes. A wire may have at most
//! two such incidences. The rewriter keeps the petgraph (and the statement →
//! node map beside it) correct incrementally; there is no public edge API.

mod surgery;
mod topology;
