use crate::arena::{GetInfo, Id};
use crate::identifier;
use crate::{Dialect, SSAValue, Statement, Symbol};

use super::port::Port;

identifier! {
    /// A unique identifier for an undirected graph body.
    struct UnGraph
}

impl std::fmt::Display for UnGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "^ug{}", self.0.raw())
    }
}

#[derive(Clone, Debug)]
pub struct UnGraphInfo<L: Dialect> {
    pub(crate) id: UnGraph,
    pub(crate) parent: Option<Statement>,
    pub(crate) name: Option<Symbol>,
    pub(crate) ports: Vec<Port>,
    pub(crate) edge_count: usize,
    pub(crate) graph: petgraph::Graph<Statement, SSAValue, petgraph::Undirected>,
    pub(crate) edge_statements: Vec<Statement>,
    _marker: std::marker::PhantomData<L>,
}

impl<L: Dialect> UnGraphInfo<L> {
    pub fn new(
        id: UnGraph,
        parent: Option<Statement>,
        name: Option<Symbol>,
        ports: Vec<Port>,
        edge_count: usize,
        graph: petgraph::Graph<Statement, SSAValue, petgraph::Undirected>,
        edge_statements: Vec<Statement>,
    ) -> Self {
        Self {
            id,
            parent,
            name,
            ports,
            edge_count,
            graph,
            edge_statements,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> UnGraph {
        self.id
    }

    pub fn parent(&self) -> Option<Statement> {
        self.parent
    }

    pub fn name(&self) -> Option<Symbol> {
        self.name
    }

    pub fn ports(&self) -> &[Port] {
        &self.ports
    }

    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub fn edge_ports(&self) -> &[Port] {
        &self.ports[..self.edge_count]
    }

    pub fn capture_ports(&self) -> &[Port] {
        &self.ports[self.edge_count..]
    }

    pub fn graph(&self) -> &petgraph::Graph<Statement, SSAValue, petgraph::Undirected> {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut petgraph::Graph<Statement, SSAValue, petgraph::Undirected> {
        &mut self.graph
    }

    pub fn edge_statements(&self) -> &[Statement] {
        &self.edge_statements
    }
}

impl<L: Dialect> GetInfo<L> for UnGraph {
    type Info = crate::arena::Item<UnGraphInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.ungraphs.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.ungraphs.get_mut(*self)
    }
}
