use crate::derived::SlotMap;
use crate::{DiGraph, Dialect, MemberRole, SSAValue, StageInfo, Statement, UnGraph};

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
        for (item, topology) in stage.nodes.digraphs.items.iter_mut().zip(self.into_iter()) {
            if !item.deleted() {
                item.install_mirror(&topology.members, &topology.edges);
            }
        }
    }
}

impl UnGraphTopologyMap {
    /// Write the mirror into every live `UnGraph`. Finalization only.
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        assert!(self.len() == stage.nodes.ungraphs.len());
        for (item, topology) in stage.nodes.ungraphs.items.iter_mut().zip(self.into_iter()) {
            if !item.deleted() {
                item.install_mirror(&topology.members, &topology.edges);
            }
        }
    }
}
