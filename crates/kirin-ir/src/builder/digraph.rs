use std::collections::HashMap;

use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::port::{Port, PortParent};
use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind, ResolutionInfo, SSAValue};
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
        let edge_count = self.ports.len();

        let mut all_ports = Vec::with_capacity(self.ports.len() + self.captures.len());
        for (index, (ty, name)) in self.ports.into_iter().enumerate() {
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = BuilderSSAInfo::new(
                port.into(),
                name.map(|n| self.stage.0.symbols.intern(n)),
                Some(ty),
                BuilderSSAKind::Port(PortParent::DiGraph(id), index),
            );
            self.stage.0.ssas.alloc(ssa);
            all_ports.push(port);
        }

        for (i, (ty, name)) in self.captures.into_iter().enumerate() {
            let index = edge_count + i;
            let port: Port = self.stage.ssas.next_id().into();
            let ssa = BuilderSSAInfo::new(
                port.into(),
                name.map(|n| self.stage.0.symbols.intern(n)),
                Some(ty),
                BuilderSSAKind::Port(PortParent::DiGraph(id), index),
            );
            self.stage.0.ssas.alloc(ssa);
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
        let mut replacements: HashMap<SSAValue, SSAValue> = HashMap::new();
        for &stmt_id in &self.nodes {
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
                            "digraph port",
                        );
                        replacements.insert(*arg, all_ports[index].into());
                    }
                    BuilderSSAKind::Unresolved(ResolutionInfo::Capture(key)) => {
                        let index = super::resolve_builder_key(
                            key,
                            all_ports.len() - edge_count,
                            &capture_name_to_index,
                            &self.stage.symbols,
                            "digraph capture",
                        );
                        replacements.insert(*arg, all_ports[edge_count + index].into());
                    }
                    _ => {}
                }
            }
        }
        // Apply replacements and delete placeholder SSAs
        for (&old, _) in &replacements {
            self.stage.0.ssas.delete(old);
        }
        for &stmt_id in &self.nodes {
            let info = &mut self.stage.0.statements[stmt_id];
            for arg in info.definition.arguments_mut() {
                if let Some(&replacement) = replacements.get(arg) {
                    *arg = replacement;
                }
            }
        }

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
            let info = &mut self.stage.0.statements[stmt_id];
            info.parent = Some(StatementParent::DiGraph(id));
        }

        let name_symbol = self.name.map(|n| self.stage.0.symbols.intern(n));
        let info = DiGraphInfo::new(
            id,
            self.parent,
            name_symbol,
            all_ports,
            edge_count,
            graph,
            self.yields,
        );
        self.stage.0.digraphs.alloc(info);
        id
    }
}
