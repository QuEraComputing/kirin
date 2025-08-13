from typing import Iterable
from dataclasses import field, dataclass

from kirin import ir
from kirin.print import Printable
from kirin.dialects import func
from kirin.print.printer import Printer


@dataclass
class CallGraph(Printable):
    """Call graph for a given [`ir.Method`][kirin.ir.Method].

    This class implements the [`kirin.graph.Graph`][kirin.graph.Graph] protocol.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    defs: dict[str, set[ir.Method]] = field(default_factory=dict)
    """Mapping from symbol names to methods."""
    backedges: dict[ir.Method, set[ir.Method]] = field(default_factory=dict)
    """Mapping from symbol names to backedges."""

    def __init__(self, mt: ir.Method):
        self.defs = {}
        self.backedges = {}
        self.__build(mt, set([]))

    def __build(self, mt: ir.Method, visited: set[ir.Method]):
        """Build the call graph for the given method."""
        if mt in visited:
            return

        visited.add(mt)
        self.defs.setdefault(mt.sym_name, set()).add(mt)

        for stmt in mt.callable_region.walk():
            if isinstance(stmt, func.Invoke):
                self.backedges.setdefault(stmt.callee, set()).add(mt)
                if stmt.callee not in visited:
                    self.__build(stmt.callee, visited)

    def get_neighbors(self, node: str) -> Iterable[str]:
        """Get the neighbors of a node in the call graph."""
        mt_set = self.defs[node]
        if len(mt_set) != 1:
            raise ValueError(f"Node {node} has multiple definitions: {mt_set}")

        (mt,) = mt_set
        return (edge.sym_name for edge in self.backedges.get(mt, set()))

    def get_edges(self) -> Iterable[tuple[str, str]]:
        """Get the edges of the call graph."""
        for node, neighbors in self.backedges.items():
            for neighbor in neighbors:
                yield node.sym_name, neighbor.sym_name

    def get_nodes(self) -> Iterable[str]:
        """Get the nodes of the call graph."""
        return self.defs.keys()

    def print_impl(self, printer: Printer) -> None:
        for idx, (caller, callee) in enumerate(self.backedges.items()):
            printer.plain_print(caller)
            printer.plain_print(" -> ")
            printer.print_seq(
                callee, delim=", ", prefix="[", suffix="]", emit=printer.plain_print
            )
            if idx < len(self.backedges) - 1:
                printer.print_newline()
