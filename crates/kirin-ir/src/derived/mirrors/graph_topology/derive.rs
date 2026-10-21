use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;
use smallvec::SmallVec;

use crate::arena::Id;
use crate::derived::SlotMap;
use crate::derived::error::{DanglingParent, Finding};
use crate::{
    DiGraph, Dialect, MemberRole, SSAKind, SSAValue, StageInfo, Statement, StatementInfo,
    StatementParent, UnGraph,
};

use super::table::{DiGraphTopologyMap, UnGraphTopologyMap};

/// Derive the mirror of every live `DiGraph`.
pub(in crate::derived) fn derive_digraphs<L: Dialect>(
    stage: &StageInfo<L>,
    findings: &mut Vec<Finding>,
) -> DiGraphTopologyMap {
    let mut table = DiGraphTopologyMap::sized_like(&stage.nodes.digraphs);
    for (stmt, item, graph) in members_of(stage, findings, as_digraph, is_live_digraph) {
        let topology = table.get_mut(graph).expect("sized like the digraph arena");
        topology.members.push((stmt, MemberRole::Node));
        for &value in item.definition.arguments() {
            if let Some(producer) = producer_in(stage, value, StatementParent::DiGraph(graph)) {
                topology.edges.push((producer, stmt, value));
            }
        }
    }
    table
}

/// Derive the mirror of every live `UnGraph`, reporting each wire read by
/// more than two node operand slots.
pub(in crate::derived) fn derive_ungraphs<L: Dialect>(
    stage: &StageInfo<L>,
    findings: &mut Vec<Finding>,
) -> UnGraphTopologyMap {
    let mut table = UnGraphTopologyMap::sized_like(&stage.nodes.ungraphs);
    let mut wires: SlotMap<UnGraph, Wires> = SlotMap::sized_like(&stage.nodes.ungraphs);

    // Boundary wires: the first `edge_count` ports of each graph.
    for (raw, item) in stage.nodes.ungraphs.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }
        let graph_wires = wires
            .get_mut(UnGraph::from(Id(raw)))
            .expect("sized like the ungraph arena");
        for &port in item.edge_ports() {
            graph_wires.insert(port.into(), SmallVec::new());
        }
    }

    // Members, and the wires their edge statements define. Collected before
    // counting endpoints, since a node may precede the edge statement whose
    // wire it reads.
    let members: Vec<_> = members_of(stage, findings, as_ungraph, is_live_ungraph).collect();
    for &(stmt, item, graph) in &members {
        let role = if item.definition.is_edge() {
            MemberRole::Edge
        } else {
            MemberRole::Node
        };
        table
            .get_mut(graph)
            .expect("sized like the ungraph arena")
            .members
            .push((stmt, role));
        if role == MemberRole::Edge {
            let graph_wires = wires.get_mut(graph).expect("sized like the ungraph arena");
            for &result in item.definition.results() {
                graph_wires.insert(result.into(), SmallVec::new());
            }
        }
    }

    // Endpoints: each node operand slot reading a wire of its graph.
    for &(stmt, item, graph) in &members {
        if item.definition.is_edge() {
            continue;
        }
        let graph_wires = wires.get_mut(graph).expect("sized like the ungraph arena");
        for (index, value) in item.definition.arguments().enumerate() {
            if let Some(occurrences) = graph_wires.get_mut(value) {
                occurrences.push((stmt, index));
            }
        }
    }

    // Two endpoints make an edge; more is invalid IR.
    for (graph, graph_wires) in wires.iter() {
        for (&value, occurrences) in graph_wires {
            match occurrences.as_slice() {
                &[(a, _), (b, _)] => table
                    .get_mut(graph)
                    .expect("sized like the ungraph arena")
                    .edges
                    .push((a, b, value)),
                _ if occurrences.len() > 2 => findings.push(Finding::UnGraphDegreeExceeded {
                    graph,
                    value,
                    occurrences: occurrences.to_vec(),
                }),
                _ => {}
            }
        }
    }
    table
}

/// Wire → the `(node, operand_index)` slots reading it, in member order.
type Wires = IndexMap<SSAValue, SmallVec<[(Statement, usize); 2]>, FxBuildHasher>;

/// The live statements parented to a graph of one kind, in id order, each
/// with the graph. A statement whose graph is not live is reported as a
/// [`DanglingParent`] and skipped.
fn members_of<'s, L: Dialect, G: Copy>(
    stage: &'s StageInfo<L>,
    findings: &'s mut Vec<Finding>,
    graph_of: fn(StatementParent) -> Option<G>,
    is_live: fn(&StageInfo<L>, G) -> bool,
) -> impl Iterator<Item = (Statement, &'s StatementInfo<L>, G)> + use<'s, L, G> {
    stage
        .nodes
        .statements
        .items
        .iter()
        .enumerate()
        .filter_map(move |(raw, item)| {
            if item.deleted() {
                return None;
            }
            let parent = item.parent?;
            let graph = graph_of(parent)?;
            let stmt = Statement::from(Id(raw));
            if !is_live(stage, graph) {
                findings.push(Finding::DanglingParent(DanglingParent::StatementInGraph {
                    stmt,
                    parent,
                }));
                return None;
            }
            Some((stmt, &**item, graph))
        })
}

/// The live statement producing `value`, if it is a result of one parented
/// to `parent`.
fn producer_in<L: Dialect>(
    stage: &StageInfo<L>,
    value: SSAValue,
    parent: StatementParent,
) -> Option<Statement> {
    let info = stage.ssas.get(value)?.data.as_ref()?;
    let SSAKind::Result(producer, _) = *info.kind() else {
        return None;
    };
    stage
        .nodes
        .statements
        .get(producer)
        .filter(|item| item.parent == Some(parent))
        .map(|_| producer)
}

fn as_digraph(parent: StatementParent) -> Option<DiGraph> {
    match parent {
        StatementParent::DiGraph(graph) => Some(graph),
        _ => None,
    }
}

fn as_ungraph(parent: StatementParent) -> Option<UnGraph> {
    match parent {
        StatementParent::UnGraph(graph) => Some(graph),
        _ => None,
    }
}

fn is_live_digraph<L: Dialect>(stage: &StageInfo<L>, graph: DiGraph) -> bool {
    stage.nodes.digraphs.get(graph).is_some()
}

fn is_live_ungraph<L: Dialect>(stage: &StageInfo<L>, graph: UnGraph) -> bool {
    stage.nodes.ungraphs.get(graph).is_some()
}
