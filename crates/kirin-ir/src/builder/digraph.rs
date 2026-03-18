use std::collections::HashMap;

use crate::arena::GetInfo;
use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::port::{Port, PortParent};
use crate::node::ssa::{BuilderSSAKind, ResolutionInfo, SSAInfo, SSAValue};
use crate::node::stmt::{Statement, StatementParent};
use crate::{Dialect, StageInfo};

pub struct DiGraphBuilder<'a, L: Dialect> {
    stage: &'a mut StageInfo<L>,
    parent: Option<Statement>,
    name: Option<String>,
    ports: Vec<(L::Type, Option<String>)>,
    captures: Vec<(L::Type, Option<String>)>,
    nodes: Vec<Statement>,
    yields: Vec<SSAValue>,
}

impl<'a, L: Dialect> DiGraphBuilder<'a, L> {
    pub(crate) fn from_stage(stage: &'a mut StageInfo<L>) -> Self {
        DiGraphBuilder {
            stage,
            parent: None,
            name: None,
            ports: Vec::new(),
            captures: Vec::new(),
            nodes: Vec::new(),
            yields: Vec::new(),
        }
    }

    /// Attach the digraph to a parent statement.
    pub fn parent(mut self, stmt: Statement) -> Self {
        self.parent = Some(stmt);
        self
    }

    /// Set the name/label of this digraph.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add an edge port with a given type.
    pub fn port<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.ports.push((ty.into(), None));
        self
    }

    /// Name the most recently added port.
    ///
    /// Must be called immediately after [`port`](Self::port).
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

    /// Add a capture port with a given type.
    pub fn capture<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.captures.push((ty.into(), None));
        self
    }

    /// Name the most recently added capture port.
    ///
    /// Must be called immediately after [`capture`](Self::capture).
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

    /// Add a statement as a graph node.
    pub fn node(mut self, stmt: Statement) -> Self {
        self.nodes.push(stmt);
        self
    }

    /// Add a yield output value.
    pub fn yield_value(mut self, ssa: SSAValue) -> Self {
        self.yields.push(ssa);
        self
    }

    /// Finalize the digraph and add it to the stage.
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> DiGraph {
        let id = self.stage.digraphs.next_id();
        let edge_count = self.ports.len();

        // Step 1: Create Port SSAValues for edge ports (indices 0..N)
        let mut all_ports = Vec::with_capacity(self.ports.len() + self.captures.len());
        for (index, (ty, name)) in self.ports.into_iter().enumerate() {
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = SSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                ty,
                BuilderSSAKind::Port(PortParent::DiGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }

        // Step 2: Create Port SSAValues for capture ports (indices N..N+M)
        for (i, (ty, name)) in self.captures.into_iter().enumerate() {
            let index = edge_count + i;
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = SSAInfo::new(
                port.into(),
                name.map(|n| self.stage.symbols.intern(n)),
                ty,
                BuilderSSAKind::Port(PortParent::DiGraph(id), index),
            );
            self.stage.ssas.alloc(ssa);
            all_ports.push(port);
        }

        // Step 3: Resolve Unresolved(Port/Capture) placeholders in node statement operands
        // Build name→index maps for port and capture lookup
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

        for &stmt_id in &self.nodes {
            let info = &mut self.stage.statements[stmt_id];
            for arg in info.definition.arguments_mut() {
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
                            "digraph port",
                        );
                        self.stage.ssas.delete(*arg);
                        *arg = all_ports[index].into();
                    }
                    BuilderSSAKind::Unresolved(ResolutionInfo::Capture(key)) => {
                        let index = super::resolve_builder_key(
                            key,
                            all_ports.len() - edge_count,
                            &capture_name_to_index,
                            &self.stage.symbols,
                            "digraph capture",
                        );
                        self.stage.ssas.delete(*arg);
                        *arg = all_ports[edge_count + index].into();
                    }
                    _ => {}
                }
            }
        }

        // Step 4: Build petgraph::DiGraph<Statement, SSAValue>
        // Map Statement -> NodeIndex for nodes in this graph
        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph = petgraph::Graph::<Statement, SSAValue, petgraph::Directed>::new();

        for &stmt_id in &self.nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }

        // For each node's operands, if the operand's producer is also in this graph, add an edge
        for &stmt_id in &self.nodes {
            let consumer_ni = stmt_to_node[&stmt_id];
            let info = stmt_id.expect_info(self.stage);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                let ssa_info = self
                    .stage
                    .ssas
                    .get(operand)
                    .expect("SSAValue not found in stage");
                if let BuilderSSAKind::Result(producer_stmt, _) = ssa_info.kind
                    && let Some(&producer_ni) = stmt_to_node.get(&producer_stmt)
                {
                    graph.add_edge(producer_ni, consumer_ni, operand);
                }
            }
        }

        // Preserve insertion order — the graph edges encode the topology
        // regardless of node iteration order. Callers can request topological
        // sort explicitly when needed.
        let nodes = self.nodes;

        // Step 5: Set StatementParent::DiGraph on all node statements
        for &stmt_id in &nodes {
            let info = &mut self.stage.statements[stmt_id];
            info.parent = Some(StatementParent::DiGraph(id));
        }

        // Step 7: Create DiGraphInfo and allocate
        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = DiGraphInfo::new(
            id,
            self.parent,
            name_symbol,
            all_ports,
            edge_count,
            graph,
            self.yields,
        );
        self.stage.digraphs.alloc(info);
        id
    }
}
