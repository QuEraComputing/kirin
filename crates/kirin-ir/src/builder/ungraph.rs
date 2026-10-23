use crate::node::port::PortParent;
use crate::node::stmt::Statement;
use crate::node::ungraph::{UnGraph, UnGraphInfo};
use crate::{BuilderStageInfo, Dialect};

/// What an `UnGraph` member is stored as: a petgraph node, or an edge
/// statement whose results are wires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnGraphRole {
    Node,
    Edge,
}

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
    /// Add an edge statement. `stmt` must be an edge statement with no
    /// operands, neither a terminator nor carrying successors.
    pub fn edge(mut self, stmt: Statement) -> Self {
        self.stage.assert_ungraph_member(stmt, UnGraphRole::Edge);
        self.edge_stmts.push(stmt);
        self
    }

    /// Add a node. `stmt` must not be an edge statement, a terminator, or
    /// carry successors.
    pub fn node(mut self, stmt: Statement) -> Self {
        self.stage.assert_ungraph_member(stmt, UnGraphRole::Node);
        self.nodes.push(stmt);
        self
    }

    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> UnGraph {
        let id = self.stage.ungraphs.next_id();

        // Allocate ports and resolve placeholders using shared helper
        let allocated = super::graph_common::allocate_ports(
            self.stage,
            self.ports,
            self.captures,
            PortParent::UnGraph(id),
        );

        // UnGraph resolves both node and edge statements
        let all_stmts: Vec<Statement> = self
            .nodes
            .iter()
            .chain(self.edge_stmts.iter())
            .copied()
            .collect();
        super::graph_common::resolve_and_replace(self.stage, &all_stmts, &allocated, "ungraph");

        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = UnGraphInfo::from_parts(
            id,
            self.parent,
            name_symbol,
            allocated.all_ports,
            allocated.edge_count,
        );
        let _ = self.stage.ungraphs.alloc(info);
        self.stage
            .add_ungraph_members(id, &self.edge_stmts, &self.nodes);
        id
    }
}
