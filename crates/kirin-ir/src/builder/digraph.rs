use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::port::PortParent;
use crate::node::ssa::SSAValue;
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

    /// Add a member statement. `stmt` must be neither a terminator nor carry
    /// block successors.
    pub fn node(mut self, stmt: Statement) -> Self {
        let info = &self.stage.statements[stmt];
        assert!(
            !info.definition.is_terminator(),
            "Cannot add terminator statement {:?} as a digraph node",
            info.definition
        );
        assert!(
            info.definition.successors().next().is_none(),
            "Cannot add statement {:?} with successors as a digraph node",
            info.definition
        );
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

        let name_symbol = self.name.map(|n| self.stage.symbols.intern(n));
        let info = DiGraphInfo::from_parts(
            id,
            self.parent,
            name_symbol,
            allocated.all_ports,
            allocated.edge_count,
            self.yields,
        );
        let _ = self.stage.digraphs.alloc(info);
        self.stage
            .add_graph_members(StatementParent::DiGraph(id), &self.nodes);
        id
    }
}
