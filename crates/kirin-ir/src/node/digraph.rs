use crate::Dialect;
use crate::arena::{GetInfo, Id};
use crate::identifier;

use super::graph::{DiGraphExtra, GraphInfo};

identifier! {
    /// A unique identifier for a directed graph body.
    struct DiGraph
}

impl std::fmt::Display for DiGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "^dg{}", self.0.raw())
    }
}

/// Information about a directed graph body.
///
/// This is a type alias for the generic [`GraphInfo`] with directed edges
/// and [`DiGraphExtra`] (yield values).
pub type DiGraphInfo<L> = GraphInfo<L, petgraph::Directed, DiGraphExtra>;

impl<L: Dialect> DiGraphInfo<L> {
    /// Construct a new `DiGraphInfo` with all fields.
    ///
    /// This constructor preserves the original API. Internally it delegates
    /// to [`GraphInfo::new`].
    pub fn from_parts(
        id: DiGraph,
        parent: Option<crate::Statement>,
        name: Option<crate::Symbol>,
        ports: Vec<super::port::Port>,
        edge_count: usize,
        graph: petgraph::Graph<crate::Statement, crate::SSAValue, petgraph::Directed>,
        yields: Vec<crate::SSAValue>,
    ) -> Self {
        GraphInfo::new(
            id.into(),
            parent,
            name,
            ports,
            edge_count,
            graph,
            DiGraphExtra::new(yields),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Id;

    #[test]
    fn test_digraph_display() {
        let dg = DiGraph(Id(0));
        assert_eq!(format!("{dg}"), "^dg0");
    }

    #[test]
    fn test_digraph_display_nonzero() {
        let dg = DiGraph(Id(42));
        assert_eq!(format!("{dg}"), "^dg42");
    }
}
