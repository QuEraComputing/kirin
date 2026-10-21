//! The graph mirror of every `DiGraph` and `UnGraph`: its member map and its
//! petgraph, derived from each statement's authoritative `parent` and
//! operands.
//!
//! - Members are the live statements parented to the graph, in id order. An
//!   `UnGraph` member is an [`Edge`](crate::GraphMember::Edge) when
//!   `is_edge()`, otherwise a node; every `DiGraph` member is a node.
//! - A `DiGraph` edge `producer → consumer` stands for one operand slot of
//!   `consumer` reading a result of `producer`, another member.
//! - An `UnGraph` wire is a result of an edge member or one of the first
//!   `edge_count` ports. Each node operand slot reading it is an endpoint; a
//!   wire with exactly two is one edge between those nodes, and a wire with
//!   more is a [`Finding::UnGraphDegreeExceeded`](crate::Finding).

mod derive;
mod table;
mod verify;

pub(in crate::derived) use derive::{derive_digraphs, derive_ungraphs};
pub(in crate::derived) use table::{DiGraphTopologyMap, UnGraphTopologyMap};
