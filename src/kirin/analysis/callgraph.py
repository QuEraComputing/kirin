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

    defs: dict[str, ir.Method] = field(default_factory=dict)
    """Mapping from symbol names to methods."""
    backedges: dict[str, set[str]] = field(default_factory=dict)
    """Mapping from symbol names to backedges."""
    name_counts: dict[str, int] = field(default_factory=dict)
    """Mapping from symbol names to counts of how many times they have been used."""
    inv_defs: dict[ir.Method, str] = field(default_factory=dict)
    """Mapping from symbol names to methods, used for inverse lookups."""

    def __init__(self, mt: ir.Method):
        self.defs = {}
        self.backedges = {}
        self.name_counts = {}
        self.inv_defs = {}
        self.__build(mt, set([]))

    def __get_name(self, mt: ir.Method) -> str:
        """Get the name of the method, accounting for overlapping symbol names."""
        if mt in self.inv_defs:
            return self.inv_defs[mt]
        else:
            count = self.name_counts.setdefault(sym_name := mt.sym_name, 0)
            if count > 0:  # this is needed to avoid breaking the previous logic
                sym_name = f"{mt.sym_name}_{count + 1}"
                self.name_counts[mt.sym_name] += 1

            self.inv_defs[mt] = sym_name
            self.defs[sym_name] = mt
            return sym_name

    def __build(self, mt: ir.Method, visited: set[str]):
        """Build the call graph for the given method."""
        sym_name = self.__get_name(mt)

        for stmt in mt.callable_region.walk():
            if isinstance(stmt, func.Invoke):
                callee_sym_name = self.__get_name(stmt.callee)
                backedges = self.backedges.setdefault(callee_sym_name, set())
                backedges.add(sym_name)
                if callee_sym_name not in visited:
                    visited.add(callee_sym_name)
                    self.__build(stmt.callee, visited)

    def get_neighbors(self, node: str) -> Iterable[str]:
        """Get the neighbors of a node in the call graph."""
        return self.backedges.get(node, ())

    def get_edges(self) -> Iterable[tuple[str, str]]:
        """Get the edges of the call graph."""
        for node, neighbors in self.backedges.items():
            for neighbor in neighbors:
                yield node, neighbor

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
