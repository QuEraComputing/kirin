use petgraph::EdgeType;

use crate::node::port::Port;
use crate::{Dialect, SSAValue, Statement, Symbol};

/// Extra data specific to directed graphs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiGraphExtra {
    pub(crate) yields: Vec<SSAValue>,
}

impl DiGraphExtra {
    /// Create a new directed graph extra with the given yield values.
    pub fn new(yields: Vec<SSAValue>) -> Self {
        Self { yields }
    }
}

/// Extra data specific to undirected graphs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UnGraphExtra {
    pub(crate) edge_statements: Vec<Statement>,
}

impl UnGraphExtra {
    /// Create a new undirected graph extra with the given edge statements.
    pub fn new(edge_statements: Vec<Statement>) -> Self {
        Self { edge_statements }
    }
}

/// Unified graph information, parameterized by edge direction and extra data.
///
/// `D` is the petgraph edge type ([`petgraph::Directed`] or [`petgraph::Undirected`]).
/// `Extra` carries direction-specific data (yields for digraphs, edge statements for ungraphs).
///
/// Use the type aliases [`DiGraphInfo`](super::DiGraphInfo) and
/// [`UnGraphInfo`](super::UnGraphInfo) rather than naming this type directly.
#[derive(Clone, Debug)]
pub struct GraphInfo<L: Dialect, D: EdgeType, Extra> {
    pub(crate) id: crate::arena::Id,
    pub(crate) parent: Option<Statement>,
    pub(crate) name: Option<Symbol>,
    pub(crate) ports: Vec<Port>,
    pub(crate) edge_count: usize,
    pub(crate) graph: petgraph::Graph<Statement, SSAValue, D>,
    pub(crate) extra: Extra,
    _marker: std::marker::PhantomData<L>,
}

impl<L: Dialect, D: EdgeType, Extra> GraphInfo<L, D, Extra> {
    /// Create a new graph info.
    pub fn new(
        id: crate::arena::Id,
        parent: Option<Statement>,
        name: Option<Symbol>,
        ports: Vec<Port>,
        edge_count: usize,
        graph: petgraph::Graph<Statement, SSAValue, D>,
        extra: Extra,
    ) -> Self {
        Self {
            id,
            parent,
            name,
            ports,
            edge_count,
            graph,
            extra,
            _marker: std::marker::PhantomData,
        }
    }

    /// The parent statement that owns this graph, if any.
    pub fn parent(&self) -> Option<Statement> {
        self.parent
    }

    /// The optional symbolic name of this graph.
    pub fn name(&self) -> Option<Symbol> {
        self.name
    }

    /// All ports (edge ports followed by capture ports).
    pub fn ports(&self) -> &[Port] {
        &self.ports
    }

    /// The number of edge ports (the first `edge_count` elements of `ports()`).
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// The edge ports (the boundary ports that connect to external edges).
    pub fn edge_ports(&self) -> &[Port] {
        &self.ports[..self.edge_count]
    }

    /// The capture ports (ports that capture values from the enclosing scope).
    pub fn capture_ports(&self) -> &[Port] {
        &self.ports[self.edge_count..]
    }

    /// A reference to the underlying petgraph.
    pub fn graph(&self) -> &petgraph::Graph<Statement, SSAValue, D> {
        &self.graph
    }

    /// A mutable reference to the underlying petgraph.
    pub fn graph_mut(&mut self) -> &mut petgraph::Graph<Statement, SSAValue, D> {
        &mut self.graph
    }

    /// A reference to the direction-specific extra data.
    pub fn extra(&self) -> &Extra {
        &self.extra
    }

    /// A mutable reference to the direction-specific extra data.
    pub fn extra_mut(&mut self) -> &mut Extra {
        &mut self.extra
    }
}

// --- Directed graph convenience accessors ---

impl<L: Dialect> GraphInfo<L, petgraph::Directed, DiGraphExtra> {
    /// The directed graph's arena ID.
    pub fn id(&self) -> super::digraph::DiGraph {
        super::digraph::DiGraph(self.id)
    }

    /// The yield values produced by this directed graph.
    pub fn yields(&self) -> &[SSAValue] {
        &self.extra.yields
    }
}

// --- Undirected graph convenience accessors ---

impl<L: Dialect> GraphInfo<L, petgraph::Undirected, UnGraphExtra> {
    /// The undirected graph's arena ID.
    pub fn id(&self) -> super::ungraph::UnGraph {
        super::ungraph::UnGraph(self.id)
    }

    /// The edge statements in BFS-canonical order.
    pub fn edge_statements(&self) -> &[Statement] {
        &self.extra.edge_statements
    }
}
