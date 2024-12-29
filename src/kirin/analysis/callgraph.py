from typing import Iterable
from dataclasses import field, dataclass

from kirin import ir
from kirin.print import Printable
from kirin.dialects import func
from kirin.print.printer import Printer


@dataclass
class CallGraph(Printable):
    defs: dict[str, ir.Method] = field(default_factory=dict)
    backedges: dict[str, set[str]] = field(default_factory=dict)

    def __init__(self, mt: ir.Method):
        self.defs = {}
        self.backedges = {}
        self.__build(mt)

    def __build(self, mt: ir.Method):
        self.defs[mt.sym_name] = mt
        for stmt in mt.callable_region.walk():
            if isinstance(stmt, func.Invoke):
                backedges = self.backedges.setdefault(stmt.callee.sym_name, set())
                backedges.add(mt.sym_name)
                self.__build(stmt.callee)

    def get_neighbors(self, node: str) -> Iterable[str]:
        return self.backedges.get(node, ())

    def get_edges(self) -> Iterable[tuple[str, str]]:
        for node, neighbors in self.backedges.items():
            for neighbor in neighbors:
                yield node, neighbor

    def get_nodes(self) -> Iterable[str]:
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
