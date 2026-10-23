//! The `UnGraph` degree rule: a wire (an edge statement's result or a boundary
//! port) is read by at most two node operand slots. This is not a mirror —
//! the rewriter lets a wire exceed it mid-pass — so it is only checked, and a
//! violation is reported as [`Finding::UnGraphDegreeExceeded`].

use rustc_hash::FxHashMap;

use crate::arena::Id;
use crate::derived::error::Finding;
use crate::{Dialect, SSAValue, StageInfo, StatementParent, UnGraph};

/// Report every wire of a live `UnGraph` read by more than two node slots.
pub(in crate::derived) fn check<L: Dialect>(stage: &StageInfo<L>, findings: &mut Vec<Finding>) {
    // Every wire, with how many node operand slots read it. Membership comes
    // from each statement's `parent`, which is authoritative.
    let mut reads: FxHashMap<(UnGraph, SSAValue), usize> = FxHashMap::default();

    // Wires from boundary ports: the first `edge_count` ports of each graph.
    for (raw, item) in stage.nodes.ungraphs.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }
        let graph = UnGraph::from(Id(raw));
        for &port in item.edge_ports() {
            reads.insert((graph, port.into()), 0);
        }
    }
    // Live statements parented to an UnGraph, walked twice below (`clone`
    // copies the iterator, not the statements).
    let members = stage
        .nodes
        .statements
        .items
        .iter()
        .filter_map(|item| match item.parent {
            Some(StatementParent::UnGraph(graph)) if !item.deleted() => Some((graph, item)),
            _ => None,
        });
    // Wires from edge statements' results. Registered before counting, since
    // a node may precede the edge statement whose wire it reads.
    for (graph, item) in members.clone() {
        if item.definition.is_edge() {
            for &result in item.definition.results() {
                reads.insert((graph, result.into()), 0);
            }
        }
    }
    // Count the node operand slots reading each wire; other operands are not
    // wires and have no entry.
    for (graph, item) in members {
        if !item.definition.is_edge() {
            for &arg in item.definition.arguments() {
                if let Some(count) = reads.get_mut(&(graph, arg)) {
                    *count += 1;
                }
            }
        }
    }

    // Report wires over the limit, sorted so findings come out in a stable
    // order.
    let mut exceeded: Vec<((UnGraph, SSAValue), usize)> =
        reads.into_iter().filter(|&(_, count)| count > 2).collect();
    exceeded.sort_by_key(|&((graph, value), _)| (graph.0.raw(), value.0.raw()));
    findings.extend(exceeded.into_iter().map(|((graph, value), endpoints)| {
        Finding::UnGraphDegreeExceeded {
            graph,
            value,
            endpoints,
        }
    }));
}
