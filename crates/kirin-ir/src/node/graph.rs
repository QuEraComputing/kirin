use indexmap::IndexMap;
use petgraph::EdgeType;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use rustc_hash::FxBuildHasher;

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

/// Extra data specific to undirected graphs. Empty: an `UnGraph`'s edge
/// statements are [`GraphMember::Edge`] entries of its member map.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnGraphExtra {}

/// How a graph member is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphMember {
    /// A petgraph node, at this index.
    Node(NodeIndex),
    /// An `UnGraph` edge statement, whose results are wires. It has no
    /// petgraph node; a `DiGraph` never has one.
    Edge,
}

impl GraphMember {
    /// The member's role, without its node index.
    pub fn role(self) -> MemberRole {
        match self {
            GraphMember::Node(_) => MemberRole::Node,
            GraphMember::Edge => MemberRole::Edge,
        }
    }
}

/// Whether a graph member is a node or an `UnGraph` edge statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemberRole {
    Node,
    Edge,
}

/// Member statement → how it is stored, in statement id order: the mirror is
/// installed in id order, inserts append newly allocated statements, and a
/// removal keeps the others in order.
pub(crate) type MemberMap = IndexMap<Statement, GraphMember, FxBuildHasher>;

/// Unified graph information, parameterized by edge direction and extra data.
///
/// `D` is the petgraph edge type ([`petgraph::Directed`] or [`petgraph::Undirected`]).
/// `Extra` carries direction-specific data (yields for digraphs, nothing for ungraphs).
///
/// `members` holds every member, nodes and `UnGraph` edge statements, with
/// O(1) lookup and insert and order-preserving O(members) removal; `graph`
/// holds the connectivity between the
/// nodes. The `graph_topology` edits keep the two in lockstep.
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
    /// Written only through the `graph_topology` edits, which keep it in
    /// lockstep with `members` (hence `pub(super)`).
    pub(super) graph: StableGraph<Statement, SSAValue, D>,
    /// Every member and how it is stored. Written only through the
    /// `graph_topology` edits, alongside `graph`. Serialized with it.
    pub(super) members: MemberMap,
    pub(crate) extra: Extra,
    _marker: std::marker::PhantomData<L>,
}

impl<L: Dialect, D: EdgeType, Extra> GraphInfo<L, D, Extra> {
    /// Create a new graph info with no members.
    pub(crate) fn new(
        id: crate::arena::Id,
        parent: Option<Statement>,
        name: Option<Symbol>,
        ports: Vec<Port>,
        edge_count: usize,
        extra: Extra,
    ) -> Self {
        Self {
            id,
            parent,
            name,
            ports,
            edge_count,
            graph: StableGraph::default(),
            members: MemberMap::default(),
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

    /// A reference to the underlying petgraph. A `StableGraph`: removing a
    /// node leaves the other nodes' indices unchanged.
    pub fn graph(&self) -> &StableGraph<Statement, SSAValue, D> {
        &self.graph
    }

    /// Raw access to the petgraph half, so tests can forge a member map and
    /// petgraph that disagree. No edit outside tests may bypass the
    /// `graph_topology` lockstep.
    #[cfg(test)]
    pub(crate) fn graph_mut(&mut self) -> &mut StableGraph<Statement, SSAValue, D> {
        &mut self.graph
    }

    /// Every member with how it is stored, in member order: statement id
    /// order, which carries no meaning but is deterministic and unchanged by
    /// removals.
    pub fn members(&self) -> impl Iterator<Item = (Statement, GraphMember)> + '_ {
        self.members.iter().map(|(&stmt, &member)| (stmt, member))
    }

    /// The first member in member order.
    pub(crate) fn first_member(&self) -> Option<Statement> {
        self.members.get_index(0).map(|(&stmt, _)| stmt)
    }

    /// The member after `stmt` in member order, in O(1).
    pub(crate) fn next_member(&self, stmt: Statement) -> Option<Statement> {
        let index = self.members.get_index_of(&stmt)?;
        self.members.get_index(index + 1).map(|(&next, _)| next)
    }

    /// How `stmt` is stored, if it is a member.
    pub fn member(&self, stmt: Statement) -> Option<GraphMember> {
        self.members.get(&stmt).copied()
    }

    /// The node members, in member order.
    pub fn nodes(&self) -> impl Iterator<Item = Statement> + '_ {
        self.members().filter_map(|(stmt, member)| match member {
            GraphMember::Node(_) => Some(stmt),
            GraphMember::Edge => None,
        })
    }

    /// The petgraph node of member statement `stmt`, if it is a node.
    pub fn node_idx_of(&self, stmt: Statement) -> Option<NodeIndex> {
        match self.member(stmt)? {
            GraphMember::Node(node) => Some(node),
            GraphMember::Edge => None,
        }
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

    /// The edge statements, in member order.
    pub fn edge_statements(&self) -> impl Iterator<Item = Statement> + '_ {
        self.members().filter_map(|(stmt, member)| match member {
            GraphMember::Edge => Some(stmt),
            GraphMember::Node(_) => None,
        })
    }
}
