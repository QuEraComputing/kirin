from typing import TYPE_CHECKING, Any, Generic, Iterable, Optional, Protocol, TypeVar

if TYPE_CHECKING:
    from kirin import ir
    from kirin.print import Printer

Node = TypeVar("Node")


class Graph(Protocol, Generic[Node]):

    def get_neighbors(self, node: Node) -> Iterable[Node]: ...
    def get_nodes(self) -> Iterable[Node]: ...
    def get_edges(self) -> Iterable[tuple[Node, Node]]: ...
    def print(
        self,
        printer: Optional["Printer"] = None,
        analysis: dict["ir.SSAValue", Any] | None = None,
    ) -> None: ...
