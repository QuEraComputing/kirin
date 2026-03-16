use crate::arena::Id;
use crate::identifier;

use super::digraph::DiGraph;
use super::ungraph::UnGraph;

identifier! {
    /// A port declaration at the boundary of a graph body.
    struct Port
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{}", self.0.raw())
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PortParent {
    DiGraph(DiGraph),
    UnGraph(UnGraph),
}
