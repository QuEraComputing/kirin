use petgraph::EdgeType;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::arena::Id;
use crate::derived::compare::multiset_eq;
use crate::derived::error::Mismatch;
use crate::{DiGraph, Dialect, GraphInfo, GraphMember, StageInfo, StatementParent, UnGraph};

use super::table::{DiGraphTopologyMap, GraphEdge, GraphTopology, UnGraphTopologyMap};

impl DiGraphTopologyMap {
    /// Compare every live `DiGraph`'s installed mirror with this derivation.
    /// Never writes to the stage.
    pub(in crate::derived) fn verify<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        out: &mut Vec<Mismatch>,
    ) {
        for (raw, item) in stage.nodes.digraphs.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let graph = DiGraph::from(Id(raw));
            let derived = self.get(graph).expect("sized like the digraph arena");
            verify_graph(StatementParent::DiGraph(graph), item, derived, out);
        }
    }
}

impl UnGraphTopologyMap {
    /// Compare every live `UnGraph`'s installed mirror with this derivation.
    /// Never writes to the stage.
    pub(in crate::derived) fn verify<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        out: &mut Vec<Mismatch>,
    ) {
        for (raw, item) in stage.nodes.ungraphs.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let graph = UnGraph::from(Id(raw));
            let derived = self.get(graph).expect("sized like the ungraph arena");
            verify_graph(StatementParent::UnGraph(graph), item, derived, out);
        }
    }
}

/// Compare one installed mirror with its derivation: members and roles as
/// sets, the member map against the petgraph's nodes, and edges as a
/// multiset over statement weights. Neither `NodeIndex` nor member order is
/// compared.
fn verify_graph<L: Dialect, D: EdgeType, Extra>(
    graph: StatementParent,
    info: &GraphInfo<L, D, Extra>,
    derived: &GraphTopology,
    out: &mut Vec<Mismatch>,
) {
    let mut installed_members: Vec<_> = info
        .members()
        .map(|(stmt, member)| (stmt, member.role()))
        .collect();
    // The derived members are already in id order.
    installed_members.sort_by_key(|&(stmt, role)| (Id::from(stmt).raw(), role));
    if installed_members != derived.members {
        out.push(Mismatch::GraphMembers {
            graph,
            installed: installed_members,
            derived: derived.members.clone(),
        });
    }

    // Each node member's index must hold that statement, and the petgraph
    // must hold no other node.
    let mut disagreeing: Vec<_> = info
        .members()
        .filter_map(|(stmt, member)| match member {
            GraphMember::Node(node) if info.graph().node_weight(node) != Some(&stmt) => Some(stmt),
            _ => None,
        })
        .collect();
    disagreeing.extend(info.graph().node_indices().filter_map(|node| {
        let stmt = info.graph()[node];
        (info.node_idx_of(stmt) != Some(node)).then_some(stmt)
    }));
    if !disagreeing.is_empty() {
        out.push(Mismatch::GraphNodeIndex {
            graph,
            statements: disagreeing,
        });
    }

    let installed_edges: Vec<GraphEdge> = info
        .graph()
        .edge_references()
        .map(|edge| {
            let graph = info.graph();
            normalized::<D>((graph[edge.source()], graph[edge.target()], *edge.weight()))
        })
        .collect();
    let derived_edges: Vec<GraphEdge> = derived
        .edges
        .iter()
        .map(|&edge| normalized::<D>(edge))
        .collect();
    if !multiset_eq(&installed_edges, &derived_edges) {
        out.push(Mismatch::GraphEdges {
            graph,
            installed: installed_edges,
            derived: derived_edges,
        });
    }
}

/// An undirected edge with its endpoints in id order, so `a — b` and `b — a`
/// compare equal; a directed edge unchanged.
fn normalized<D: EdgeType>((a, b, value): GraphEdge) -> GraphEdge {
    if !D::is_directed() && Id::from(b).raw() < Id::from(a).raw() {
        (b, a, value)
    } else {
        (a, b, value)
    }
}
