from typing import Iterable
from functools import cached_property
from dataclasses import dataclass

from kirin import ir
from kirin.print import Printer, Printable
from kirin.worklist import WorkList


@dataclass
class CFG(Printable):
    """Control Flow Graph of a given IR statement.

    This class implements the [`kirin.graph.Graph`][kirin.graph.Graph] protocol.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    parent: ir.Region
    """Parent IR statement.
    """
    entry: ir.Block | None = None
    """Entry block of the CFG.
    """

    def __post_init__(self):
        if self.parent.blocks.isempty():
            self.entry = None
        else:
            self.entry = self.parent.blocks[0]

    @cached_property
    def predecessors(self):
        """CFG data, mapping a block to its predecessors."""
        graph: dict[ir.Block, set[ir.Block]] = {}
        for block, neighbors in self.successors.items():
            for neighbor in neighbors:
                graph.setdefault(neighbor, set()).add(block)
        return graph

    @cached_property
    def successors(self):
        """CFG data, mapping a block to its neighbors."""
        graph: dict[ir.Block, set[ir.Block]] = {}
        visited: set[ir.Block] = set()
        worklist: WorkList[ir.Block] = WorkList()
        if self.parent.blocks.isempty():
            return graph

        block = self.entry
        while block is not None:
            neighbors = graph.setdefault(block, set())
            if block.last_stmt is not None:
                neighbors.update(block.last_stmt.successors)
                worklist.extend(block.last_stmt.successors)
            visited.add(block)

            block = worklist.pop()
            while block is not None and block in visited:
                block = worklist.pop()
        return graph

    # graph interface
    def get_neighbors(self, node: ir.Block) -> Iterable[ir.Block]:
        return self.successors[node]

    def get_edges(self) -> Iterable[tuple[ir.Block, ir.Block]]:
        for block, neighbors in self.successors.items():
            for neighbor in neighbors:
                yield block, neighbor

    def get_nodes(self) -> Iterable[ir.Block]:
        return self.successors.keys()

    # printable interface
    def print_impl(self, printer: Printer) -> None:
        # NOTE: this make sure we use the same name
        # as the printing of CFG parent.
        with printer.string_io():
            self.parent.print(printer)

        printer.plain_print("Successors:")
        printer.print_newline()
        for block, neighbors in self.successors.items():
            printer.plain_print(f"{printer.state.block_id[block]} -> ", end="")
            printer.print_seq(
                neighbors,
                delim=", ",
                prefix="[",
                suffix="]",
                emit=lambda block: printer.plain_print(printer.state.block_id[block]),
            )
            printer.print_newline()

        if self.predecessors:
            printer.print_newline()
            printer.plain_print("Predecessors:")
            printer.print_newline()
            for block, neighbors in self.predecessors.items():
                printer.plain_print(f"{printer.state.block_id[block]} <- ", end="")
                printer.print_seq(
                    neighbors,
                    delim=", ",
                    prefix="[",
                    suffix="]",
                    emit=lambda block: printer.plain_print(
                        printer.state.block_id[block]
                    ),
                )
                printer.print_newline()
