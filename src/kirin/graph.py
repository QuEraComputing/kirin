from typing import TYPE_CHECKING, Any, Generic, TypeVar, Iterable, Optional, Protocol

if TYPE_CHECKING:
    from kirin import ir
    from kirin.print import Printer

Node = TypeVar("Node")


class Graph(Protocol, Generic[Node]):
    """The graph interface.

    This interface defines the methods that a graph object must implement.
    The graph interface is mainly for compatibility reasons so that one can
    use multiple graph implementations interchangeably.
    """

    def get_neighbors(self, node: Node) -> Iterable[Node]:
        """Get the neighbors of a node."""
        ...

    def get_nodes(self) -> Iterable[Node]:
        """Get all the nodes in the graph."""
        ...

    def get_edges(self) -> Iterable[tuple[Node, Node]]:
        """Get all the edges in the graph."""
        ...

    def print(
        self,
        printer: Optional["Printer"] = None,
        analysis: dict["ir.SSAValue", Any] | None = None,
    ) -> None: ...
