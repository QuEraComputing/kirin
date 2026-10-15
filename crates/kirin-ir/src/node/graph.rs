use petgraph::EdgeType;
use petgraph::graph::NodeIndex;
use rustc_hash::FxHashMap;

use crate::node::port::Port;
use crate::{Dialect, SSAValue, Statement, Symbol};

/// Extra data specific to directed graphs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiGraphExtra {
    pub(crate) yields: Vec<SSAValue>,
}

impl DiGraphExtra {
    /// Create a new directed graph extra with the given yield values.
    pub(crate) fn new(yields: Vec<SSAValue>) -> Self {
        Self { yields }
    }
}

/// Extra data specific to undirected graphs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnGraphExtra {
    pub(crate) edge_statements: Vec<Statement>,
}

impl UnGraphExtra {
    /// Create a new undirected graph extra with the given edge statements.
    pub(crate) fn new(edge_statements: Vec<Statement>) -> Self {
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(
        serialize = "Extra: serde::Serialize",
        deserialize = "Extra: serde::Deserialize<'de>"
    ))
)]
pub struct GraphInfo<L: Dialect, D: EdgeType, Extra> {
    pub(crate) id: crate::arena::Id,
    pub(crate) parent: Option<Statement>,
    pub(crate) name: Option<Symbol>,
    pub(crate) ports: Vec<Port>,
    pub(crate) edge_count: usize,
    pub(crate) graph: petgraph::Graph<Statement, SSAValue, D>,
    /// Member statement → its node in `graph`. Kept in lockstep with `graph`
    /// by the rewriter; not checked by `derived/` verification.
    pub(crate) stmt_to_node_idx: FxHashMap<Statement, NodeIndex>,
    pub(crate) extra: Extra,
    _marker: std::marker::PhantomData<L>,
}

impl<L: Dialect, D: EdgeType, Extra> GraphInfo<L, D, Extra> {
    /// Create a new graph info.
    pub(crate) fn new(
        id: crate::arena::Id,
        parent: Option<Statement>,
        name: Option<Symbol>,
        ports: Vec<Port>,
        edge_count: usize,
        graph: petgraph::Graph<Statement, SSAValue, D>,
        extra: Extra,
    ) -> Self {
        let stmt_to_node_idx = graph
            .node_indices()
            .map(|node| (graph[node], node))
            .collect();
        Self {
            id,
            parent,
            name,
            ports,
            edge_count,
            graph,
            stmt_to_node_idx,
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

    /// The petgraph node of member statement `stmt`, if it is a member.
    pub fn node_idx_of(&self, stmt: Statement) -> Option<NodeIndex> {
        self.stmt_to_node_idx.get(&stmt).copied()
    }

    /// A reference to the direction-specific extra data.
    pub fn extra(&self) -> &Extra {
        &self.extra
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

    /// Mutable access to the yield slots, for the rewriter to replace a
    /// yielded value in place while maintaining the def-use index.
    pub(crate) fn yields_mut(&mut self) -> &mut [SSAValue] {
        &mut self.extra.yields
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
