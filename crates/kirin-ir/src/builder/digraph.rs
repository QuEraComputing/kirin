use std::collections::HashMap;

use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::port::PortParent;
use crate::node::ssa::{BuilderSSAKind, SSAValue};
use crate::node::stmt::{Statement, StatementParent};
use crate::{BuilderStageInfo, Dialect};

pub struct DiGraphBuilder<'a, L: Dialect> {
    stage: &'a mut BuilderStageInfo<L>,
    parent: Option<Statement>,
    name: Option<String>,
    ports: Vec<(L::Type, Option<String>)>,
    captures: Vec<(L::Type, Option<String>)>,
    nodes: Vec<Statement>,
    yields: Vec<SSAValue>,
}

impl<'a, L: Dialect> DiGraphBuilder<'a, L> {
    pub(crate) fn from_stage(stage: &'a mut BuilderStageInfo<L>) -> Self {
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

    pub fn node(mut self, stmt: Statement) -> Self {
        self.nodes.push(stmt);
        self
    }

    pub fn yield_value(mut self, ssa: SSAValue) -> Self {
        self.yields.push(ssa);
        self
    }

    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> DiGraph {
        let id = self.stage.digraphs.next_id();

        // Allocate ports and resolve placeholders using shared helper
        let allocated = super::graph_common::allocate_ports(
            self.stage,
            self.ports,
            self.captures,
            PortParent::DiGraph(id),
        );
        super::graph_common::resolve_and_replace(self.stage, &self.nodes, &allocated, "digraph");

        // Build the directed petgraph
        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph = petgraph::Graph::<Statement, SSAValue, petgraph::Directed>::new();

        for &stmt_id in &self.nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }

        for &stmt_id in &self.nodes {
            let consumer_ni = stmt_to_node[&stmt_id];
            let info = &self.stage.statements[stmt_id];
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

        let nodes = self.nodes;
        for &stmt_id in &nodes {
            let info = &mut self.stage.statements[stmt_id];
            info.parent = Some(StatementParent::DiGraph(id));
        }

        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = DiGraphInfo::from_parts(
            id,
            self.parent,
            name_symbol,
            allocated.all_ports,
            allocated.edge_count,
            graph,
            self.yields,
        );
        let _ = self.stage.digraphs.alloc(info);
        id
    }
}
