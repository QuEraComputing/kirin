use crate::Dialect;
use crate::arena::{GetInfo, Id};
use crate::identifier;

use super::graph::{GraphInfo, UnGraphExtra};

identifier! {
    /// A unique identifier for an undirected graph body.
    struct UnGraph
}

impl std::fmt::Display for UnGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "^ug{}", self.0.raw())
    }
}

/// Information about an undirected graph body.
///
/// This is a type alias for the generic [`GraphInfo`] with undirected edges
/// and [`UnGraphExtra`] (edge statements).
pub type UnGraphInfo<L> = GraphInfo<L, petgraph::Undirected, UnGraphExtra>;

impl<L: Dialect> UnGraphInfo<L> {
    /// Construct a new `UnGraphInfo` with all fields.
    ///
    /// This constructor preserves the original API. Internally it delegates
    /// to [`GraphInfo::new`].
    pub fn from_parts(
        id: UnGraph,
        parent: Option<crate::Statement>,
        name: Option<crate::Symbol>,
        ports: Vec<super::port::Port>,
        edge_count: usize,
        graph: petgraph::Graph<crate::Statement, crate::SSAValue, petgraph::Undirected>,
        edge_statements: Vec<crate::Statement>,
    ) -> Self {
        GraphInfo::new(
            id.into(),
            parent,
            name,
            ports,
            edge_count,
            graph,
            UnGraphExtra::new(edge_statements),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Id;

    #[test]
    fn test_ungraph_display() {
        let ug = UnGraph(Id(0));
        assert_eq!(format!("{ug}"), "^ug0");
    }

    #[test]
    fn test_ungraph_display_nonzero() {
        let ug = UnGraph(Id(7));
        assert_eq!(format!("{ug}"), "^ug7");
    }
}
