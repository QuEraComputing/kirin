use std::collections::{HashMap, HashSet, VecDeque};

use crate::node::port::{Port, PortParent};
use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind, ResolutionInfo, SSAValue};
use crate::node::stmt::{Statement, StatementParent};
use crate::node::ungraph::{UnGraph, UnGraphInfo};
use crate::{BuilderStageInfo, Dialect};

pub struct UnGraphBuilder<'a, L: Dialect> {
    stage: &'a mut BuilderStageInfo<L>,
    parent: Option<Statement>,
    name: Option<String>,
    ports: Vec<(L::Type, Option<String>)>,
    captures: Vec<(L::Type, Option<String>)>,
    edge_stmts: Vec<Statement>,
    nodes: Vec<Statement>,
}

impl<'a, L: Dialect> UnGraphBuilder<'a, L> {
    pub(crate) fn from_stage(stage: &'a mut BuilderStageInfo<L>) -> Self {
        UnGraphBuilder {
            stage,
            parent: None,
            name: None,
            ports: Vec::new(),
            captures: Vec::new(),
            edge_stmts: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub fn parent(mut self, stmt: Statement) -> Self {
        self.parent = Some(stmt);
        self
    }
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn port<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.ports.push((ty.into(), None));
        self
    }
    pub fn port_name<S: Into<String>>(mut self, name: S) -> Self {
        debug_assert!(
            !self.ports.is_empty(),
            "port_name called without a preceding port()"
        );
        if let Some(last) = self.ports.last_mut() {
            last.1 = Some(name.into());
        }
        self
    }
    pub fn capture<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.captures.push((ty.into(), None));
        self
    }
    pub fn capture_name<S: Into<String>>(mut self, name: S) -> Self {
        debug_assert!(
            !self.captures.is_empty(),
            "capture_name called without a preceding capture()"
        );
        if let Some(last) = self.captures.last_mut() {
            last.1 = Some(name.into());
        }
        self
    }
    pub fn edge(mut self, stmt: Statement) -> Self {
        self.edge_stmts.push(stmt);
        self
    }
    pub fn node(mut self, stmt: Statement) -> Self {
        self.nodes.push(stmt);
        self
    }

    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> UnGraph {
        let id = self.stage.ungraphs.next_id();
        let edge_count = self.ports.len();

        let mut all_ports = Vec::with_capacity(self.ports.len() + self.captures.len());
        for (index, (ty, name)) in self.ports.into_iter().enumerate() {
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = BuilderSSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                Some(ty),
                BuilderSSAKind::Port(PortParent::UnGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }
        for (i, (ty, name)) in self.captures.into_iter().enumerate() {
            let index = edge_count + i;
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = BuilderSSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                Some(ty),
                BuilderSSAKind::Port(PortParent::UnGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }

        let port_name_to_index: std::collections::HashMap<crate::Symbol, usize> = all_ports
            [..edge_count]
            .iter()
            .enumerate()
            .filter_map(|(i, port)| {
                let info = self.stage.ssas.get(SSAValue::from(*port))?;
                info.name().map(|sym| (sym, i))
            })
            .collect();
        let capture_name_to_index: std::collections::HashMap<crate::Symbol, usize> = all_ports
            [edge_count..]
            .iter()
            .enumerate()
            .filter_map(|(i, port)| {
                let info = self.stage.ssas.get(SSAValue::from(*port))?;
                info.name().map(|sym| (sym, i))
            })
            .collect();

        // Build replacement map: placeholder SSA -> resolved port SSA
        let all_stmts: Vec<Statement> = self
            .nodes
            .iter()
            .chain(self.edge_stmts.iter())
            .copied()
            .collect();
        let mut replacements: std::collections::HashMap<SSAValue, SSAValue> =
            std::collections::HashMap::new();
        for &stmt_id in &all_stmts {
            let info = &self.stage.statements[stmt_id];
            for arg in info.definition.arguments() {
                if replacements.contains_key(arg) {
                    continue;
                }
                let ssa_info = self
                    .stage
                    .ssas
                    .get(*arg)
                    .expect("SSAValue not found in stage");
                match ssa_info.kind {
                    BuilderSSAKind::Unresolved(ResolutionInfo::Port(key)) => {
                        let index = super::resolve_builder_key(
                            key,
                            edge_count,
                            &port_name_to_index,
                            &self.stage.symbols,
                            "ungraph port",
                        );
                        replacements.insert(*arg, all_ports[index].into());
                    }
                    BuilderSSAKind::Unresolved(ResolutionInfo::Capture(key)) => {
                        let index = super::resolve_builder_key(
                            key,
                            all_ports.len() - edge_count,
                            &capture_name_to_index,
                            &self.stage.symbols,
                            "ungraph capture",
                        );
                        replacements.insert(*arg, all_ports[edge_count + index].into());
                    }
                    _ => {}
                }
            }
        }
        for (&old, _) in &replacements {
            self.stage.ssas.delete(old);
        }
        for &stmt_id in &all_stmts {
            let info = &mut self.stage.statements[stmt_id];
            for arg in info.definition.arguments_mut() {
                if let Some(&replacement) = replacements.get(arg) {
                    *arg = replacement;
                }
            }
        }

        let mut edge_ssa_set: HashSet<SSAValue> = HashSet::new();
        for &edge_stmt in &self.edge_stmts {
            let info = &self.stage.statements[edge_stmt];
            for result in info.definition.results() {
                edge_ssa_set.insert((*result).into());
            }
        }
        let boundary_ssa_set: HashSet<SSAValue> = all_ports
            .iter()
            .take(edge_count)
            .map(|p| (*p).into())
            .collect();

        let mut edge_ssa_to_nodes: HashMap<SSAValue, Vec<Statement>> = HashMap::new();
        for &node_stmt in &self.nodes {
            let info = &self.stage.statements[node_stmt];
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                if edge_ssa_set.contains(&operand) || boundary_ssa_set.contains(&operand) {
                    edge_ssa_to_nodes
                        .entry(operand)
                        .or_default()
                        .push(node_stmt);
                }
            }
        }

        for (ssa, nodes) in &edge_ssa_to_nodes {
            if nodes.len() > 2 {
                panic!(
                    "UnGraph constraint violated: edge SSAValue {} is used by {} node statements (max 2 allowed for undirected graph edges)",
                    ssa,
                    nodes.len()
                );
            }
        }

        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph =
            petgraph::Graph::<Statement, SSAValue, petgraph::Undirected>::new_undirected();
        for &stmt_id in &self.nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }
        for (ssa, nodes) in &edge_ssa_to_nodes {
            if nodes.len() == 2 {
                let n0 = stmt_to_node[&nodes[0]];
                let n1 = stmt_to_node[&nodes[1]];
                graph.add_edge(n0, n1, *ssa);
            }
        }

        let mut visited_nodes: HashSet<petgraph::graph::NodeIndex> = HashSet::new();
        let mut visited_edges: HashSet<SSAValue> = HashSet::new();
        let mut bfs_node_order: Vec<petgraph::graph::NodeIndex> = Vec::new();
        let mut bfs_edge_order: Vec<Statement> = Vec::new();
        let mut queue: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();

        let mut ssa_to_edge_stmt: HashMap<SSAValue, Statement> = HashMap::new();
        for &edge_stmt in &self.edge_stmts {
            let info = &self.stage.statements[edge_stmt];
            for result in info.definition.results() {
                ssa_to_edge_stmt.insert((*result).into(), edge_stmt);
            }
        }

        for &node_stmt in &self.nodes {
            let info = &self.stage.statements[node_stmt];
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            let uses_boundary = operands.iter().any(|op| boundary_ssa_set.contains(op));
            if uses_boundary {
                let ni = stmt_to_node[&node_stmt];
                if visited_nodes.insert(ni) {
                    queue.push_back(ni);
                    bfs_node_order.push(ni);
                }
            }
        }

        while let Some(ni) = queue.pop_front() {
            let stmt = graph[ni];
            let info = &self.stage.statements[stmt];
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                if !visited_edges.contains(&operand) && edge_ssa_set.contains(&operand) {
                    visited_edges.insert(operand);
                    if let Some(&edge_stmt) = ssa_to_edge_stmt.get(&operand) {
                        bfs_edge_order.push(edge_stmt);
                    }
                    if let Some(nodes) = edge_ssa_to_nodes.get(&operand) {
                        for &other_stmt in nodes {
                            let other_ni = stmt_to_node[&other_stmt];
                            if visited_nodes.insert(other_ni) {
                                queue.push_back(other_ni);
                                bfs_node_order.push(other_ni);
                            }
                        }
                    }
                }
            }
        }

        for &stmt_id in &self.nodes {
            let ni = stmt_to_node[&stmt_id];
            if visited_nodes.insert(ni) {
                bfs_node_order.push(ni);
            }
        }
        let bfs_edge_set: HashSet<Statement> = bfs_edge_order.iter().copied().collect();
        for &edge_stmt in &self.edge_stmts {
            if !bfs_edge_set.contains(&edge_stmt) {
                bfs_edge_order.push(edge_stmt);
            }
        }

        let mut new_graph =
            petgraph::Graph::<Statement, SSAValue, petgraph::Undirected>::new_undirected();
        let mut old_to_new: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> =
            HashMap::new();
        let mut reordered_nodes = Vec::with_capacity(bfs_node_order.len());
        for &old_ni in &bfs_node_order {
            let stmt = graph[old_ni];
            let new_ni = new_graph.add_node(stmt);
            old_to_new.insert(old_ni, new_ni);
            reordered_nodes.push(stmt);
        }
        for edge in graph.edge_indices() {
            let (src, dst) = graph.edge_endpoints(edge).unwrap();
            let weight = graph[edge];
            new_graph.add_edge(old_to_new[&src], old_to_new[&dst], weight);
        }

        let graph = new_graph;
        let edge_stmts = bfs_edge_order;

        for &stmt_id in &reordered_nodes {
            self.stage.statements[stmt_id].parent = Some(StatementParent::UnGraph(id));
        }
        for &stmt_id in &edge_stmts {
            self.stage.statements[stmt_id].parent = Some(StatementParent::UnGraph(id));
        }

        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = UnGraphInfo::new(
            id,
            self.parent,
            name_symbol,
            all_ports,
            edge_count,
            graph,
            edge_stmts,
        );
        self.stage.ungraphs.alloc(info);
        id
    }
}
