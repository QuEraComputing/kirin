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
    
    @cached_property
    def dominators(self):
        """Compute the dominator sets for each block in the CFG."""
        doms: dict[ir.Block, set[ir.Block]] = {}
        blocks = list(self.successors.keys())
        if not blocks:
            return doms

        entry = self.entry
        for block in blocks:
            doms[block] = set(blocks)  # Initialize to all blocks
        doms[entry] = {entry}  # Entry block dominates itself

        changed = True
        while changed:
            changed = False
            for block in blocks:
                if block == entry:
                    continue
                new_doms = set(blocks)
                for pred in self.predecessors.get(block, []):
                    new_doms &= doms[pred]
                new_doms.add(block)
                if new_doms != doms[block]:
                    doms[block] = new_doms
                    changed = True
        return doms
    
    @cached_property
    def dominator_tree(self):
        """Compute the dominator tree for the CFG."""
        idoms: dict[ir.Block, ir.Block] = {}
        doms = self.dominators
        for b in doms:
            if b == self.entry:
                continue
            idom_candidates = doms[b] - {b}
            idom = None
            for candidate in idom_candidates:
                if all((other == candidate or other not in doms[b]) for other in idom_candidates):
                    idom = candidate
                    break
            if idom is not None:
                idoms[b] = idom
        return idoms
    
    def get_nearest_common_dominator(self, block1: ir.Block, block2: ir.Block) -> ir.Block | None:
        """Get the nearest common dominator of two blocks."""
        doms1 = self.dominators.get(block1, set())
        doms2 = self.dominators.get(block2, set())
        common_doms = doms1 & doms2
        if not common_doms:
            return None
        # Find the nearest common dominator
        for dom in common_doms:
            if all((other == dom or dom not in self.dominators[other]) for other in common_doms):
                return dom
        return None 

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
