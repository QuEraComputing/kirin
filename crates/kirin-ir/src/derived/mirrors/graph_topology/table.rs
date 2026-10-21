use petgraph::EdgeType;

use crate::derived::SlotMap;
use crate::{DiGraph, Dialect, GraphInfo, MemberRole, SSAValue, StageInfo, Statement, UnGraph};

/// One edge of a graph mirror: its endpoints (from the first to the second if
/// directed) and the value it carries.
pub(in crate::derived) type GraphEdge = (Statement, Statement, SSAValue);

/// One graph's expected mirror.
#[derive(Clone, Debug, Default)]
pub(in crate::derived) struct GraphTopology {
    /// Every member with its role, in id order.
    pub(super) members: Vec<(Statement, MemberRole)>,
    /// Every edge between node members, one per operand slot (`DiGraph`) or
    /// per two-endpoint wire (`UnGraph`).
    pub(super) edges: Vec<GraphEdge>,
}

/// The expected mirror of every `DiGraph`; tombstoned slots stay empty.
pub(in crate::derived) type DiGraphTopologyMap = SlotMap<DiGraph, GraphTopology>;

/// The expected mirror of every `UnGraph`; tombstoned slots stay empty.
pub(in crate::derived) type UnGraphTopologyMap = SlotMap<UnGraph, GraphTopology>;

impl DiGraphTopologyMap {
    /// Write the mirror into every live `DiGraph`. Finalization only.
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        assert!(self.len() == stage.nodes.digraphs.len());
        install_all(stage.nodes.digraphs.items.iter_mut(), self.into_iter());
    }
}

impl UnGraphTopologyMap {
    /// Write the mirror into every live `UnGraph`. Finalization only.
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        assert!(self.len() == stage.nodes.ungraphs.len());
        install_all(stage.nodes.ungraphs.items.iter_mut(), self.into_iter());
    }
}

fn install_all<'s, L: Dialect + 's, D: EdgeType + 's, Extra: 's>(
    items: impl Iterator<Item = &'s mut crate::arena::Item<GraphInfo<L, D, Extra>>>,
    topologies: impl Iterator<Item = GraphTopology>,
) {
    for (item, topology) in items.zip(topologies) {
        if !item.deleted() {
            item.install_mirror(&topology.members, &topology.edges);
        }
    }
}
