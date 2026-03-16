use crate::arena::{GetInfo, Id};
use crate::identifier;
use crate::{Dialect, SSAValue, Statement, Symbol};

use super::port::Port;

identifier! {
    /// A unique identifier for a directed graph body.
    struct DiGraph
}

impl std::fmt::Display for DiGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "^dg{}", self.0.raw())
    }
}

#[derive(Clone, Debug)]
pub struct DiGraphInfo<L: Dialect> {
    pub(crate) id: DiGraph,
    pub(crate) parent: Option<Statement>,
    pub(crate) name: Option<Symbol>,
    pub(crate) ports: Vec<Port>,
    pub(crate) edge_count: usize,
    pub(crate) graph: petgraph::Graph<Statement, SSAValue, petgraph::Directed>,
    pub(crate) yields: Vec<SSAValue>,
    _marker: std::marker::PhantomData<L>,
}

impl<L: Dialect> DiGraphInfo<L> {
    pub fn new(
        id: DiGraph,
        parent: Option<Statement>,
        name: Option<Symbol>,
        ports: Vec<Port>,
        edge_count: usize,
        graph: petgraph::Graph<Statement, SSAValue, petgraph::Directed>,
        yields: Vec<SSAValue>,
    ) -> Self {
        Self {
            id,
            parent,
            name,
            ports,
            edge_count,
            graph,
            yields,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> DiGraph {
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

    pub fn graph(&self) -> &petgraph::Graph<Statement, SSAValue, petgraph::Directed> {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut petgraph::Graph<Statement, SSAValue, petgraph::Directed> {
        &mut self.graph
    }

    pub fn yields(&self) -> &[SSAValue] {
        &self.yields
    }
}

impl<L: Dialect> GetInfo<L> for DiGraph {
    type Info = crate::arena::Item<DiGraphInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.digraphs.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.digraphs.get_mut(*self)
    }
}
