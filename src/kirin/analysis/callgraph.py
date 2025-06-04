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

    edges: dict[ir.Method, set[ir.Method]] = field(default_factory=dict)
    """Mapping from symbol names to edges (caller -> callee)."""
    backedges: dict[ir.Method, set[ir.Method]] = field(default_factory=dict)
    """Mapping from symbol names to backedges (callee -> caller)."""

    def __init__(self, mt: ir.Method):
        self.defs = {}
        self.edges = {}
        self.backedges = {}
        self.__build(mt)

    def __build(self, mt: ir.Method):
        for stmt in mt.callable_region.walk():
            if isinstance(stmt, func.Invoke):
                edges = self.edges.setdefault(stmt.callee, set())
                edges.add(mt)
                self.__build(stmt.callee)

        for caller in self.edges:
            for callee in self.edges[caller]:
                backedges = self.backedges.setdefault(callee, set())
                backedges.add(caller)

    def get_neighbors(self, node: ir.Method) -> Iterable[ir.Method]:
        """Get the neighbors of a node in the call graph."""
        return self.edges.get(node, ())

    def get_edges(self) -> Iterable[tuple[ir.Method, ir.Method]]:
        """Get the edges of the call graph."""
        for node, neighbors in self.edges.items():
            for neighbor in neighbors:
                yield node, neighbor

    def get_nodes(self) -> Iterable[ir.Method]:
        """Get the nodes of the call graph."""
        return self.edges.keys()

    def print_impl(self, printer: Printer) -> None:
        for idx, (caller, callee) in enumerate(self.edges.items()):
            printer.plain_print(caller)
            printer.plain_print(" -> ")
            printer.print_seq(
                callee, delim=", ", prefix="[", suffix="]", emit=printer.plain_print
            )
            if idx < len(self.edges) - 1:
                printer.print_newline()
