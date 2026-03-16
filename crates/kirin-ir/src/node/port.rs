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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Id;

    #[test]
    fn test_port_display() {
        let p = Port(Id(3));
        assert_eq!(format!("{p}"), "%3");
    }

    #[test]
    fn test_port_parent_digraph() {
        let dg = DiGraph(Id(1));
        let parent = PortParent::DiGraph(dg);
        assert!(matches!(parent, PortParent::DiGraph(_)));
    }

    #[test]
    fn test_port_parent_ungraph() {
        let ug = UnGraph(Id(2));
        let parent = PortParent::UnGraph(ug);
        assert!(matches!(parent, PortParent::UnGraph(_)));
    }

    #[test]
    fn test_port_parent_equality() {
        let p1 = PortParent::DiGraph(DiGraph(Id(1)));
        let p2 = PortParent::UnGraph(UnGraph(Id(1)));
        assert_ne!(p1, p2);
    }
}
