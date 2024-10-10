from __future__ import annotations

from typing import TYPE_CHECKING, Iterable, Iterator

from typing_extensions import Self

from kirin.ir.derive import derive, field
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.block import Block
from kirin.ir.nodes.view import MutableSequenceView
from kirin.ir.ssa import SSAValue

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement
    from kirin.print import Printer


@derive(init=True, repr=True)
class RegionBlocks(MutableSequenceView[list, "Region", Block]):

    def __setitem__(
        self, idx: int | slice, block_or_blocks: Block | Iterable[Block]
    ) -> None:
        if isinstance(idx, int) and isinstance(block_or_blocks, Block):
            self.field[idx].detach()
            block_or_blocks.attach(self.node)
            self.field[idx] = block_or_blocks
            self.node._block_idx[block_or_blocks] = idx
        elif isinstance(idx, slice) and isinstance(block_or_blocks, Iterable):
            for block in block_or_blocks:
                block.attach(self.node)
            self.field[idx] = block_or_blocks
            self.node._block_idx = {
                block: i for i, block in enumerate(self.field)
            }  # reindex
        else:
            raise ValueError("Invalid assignment")

    def __delitem__(self, idx: int) -> None:
        self.field[idx].detach()

    def insert(self, idx: int, value: Block) -> None:
        value.attach(self.node)
        self.field.insert(idx, value)
        for i, value in enumerate(self.field[idx:], idx):
            self.node._block_idx[value] = i

    def append(self, value: Block) -> None:
        value.attach(self.node)
        self.node._block_idx[value] = len(self.field)
        self.field.append(value)


@derive(id_hash=True)
class Region(IRNode["Statement"]):
    _blocks: list[Block] = field(default_factory=list, repr=False)
    _block_idx: dict[Block, int] = field(default_factory=dict, repr=False)
    _parent: Statement | None = field(default=None, repr=False)

    def __init__(
        self,
        blocks: Block | Iterable[Block] = (),
        parent: Statement | None = None,
    ):
        self._blocks = []
        self._block_idx = {}
        self.parent_node = parent
        if isinstance(blocks, Block):
            blocks = (blocks,)
        for block in blocks:
            self.blocks.append(block)

    def __getitem__(self, block: Block) -> int:
        if block.parent is not self:
            raise ValueError("Block does not belong to the region")
        return self._block_idx[block]

    def clone(self):
        ret = Region()
        successor_map: dict[Block, Block] = {}
        for block in self.blocks:
            new_block = block.clone()
            successor_map[block] = new_block
            ret.blocks.append(new_block)

        for block in ret.blocks:
            if block.last_stmt is not None:
                block.last_stmt.successors = [
                    successor_map[successor] for successor in block.last_stmt.successors
                ]

        return ret

    @property
    def parent_node(self) -> Statement | None:
        return self._parent

    @parent_node.setter
    def parent_node(self, parent: Statement | None) -> None:
        from kirin.ir.nodes.stmt import Statement

        self.assert_parent(Statement, parent)
        self._parent = parent

    @property
    def blocks(self) -> RegionBlocks:
        return RegionBlocks(self, self._blocks)

    @property
    def region_index(self) -> int:
        if self.parent_node is None:
            raise ValueError("Region has no parent")
        for idx, region in enumerate(self.parent_node.regions):
            if region is self:
                return idx
        raise ValueError("Region not found in parent")

    def detach(self, index: int | None = None) -> None:
        # already detached
        if self.parent_node is None:
            return

        if index is not None:
            region_idx = index
        else:
            region_idx = self.region_index

        del self.parent_node._regions[region_idx]
        self.parent_node = None

    def drop_all_references(self) -> None:
        self.parent_node = None
        for block in self._blocks:
            block.drop_all_references()

    def delete(self, safe: bool = True) -> None:
        self.detach()
        self.drop_all_references()

    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool:
        if context is None:
            context = {}

        if len(self.blocks) != len(other.blocks):
            return False

        for block, other_block in zip(self.blocks, other.blocks):
            context[block] = other_block

        if not all(
            block.is_structurally_equal(other_block, context)
            for block, other_block in zip(self.blocks, other.blocks)
        ):
            return False

        return True

    def walk(
        self, *, reverse: bool = False, region_first: bool = False
    ) -> Iterator[Statement]:
        for block in reversed(self.blocks) if reverse else self.blocks:
            yield from block.walk(reverse=reverse, region_first=region_first)

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("{")
        if len(self.blocks) == 0:
            printer.print_newline()
            printer.plain_print("}")
            return

        result_width = 0
        for bb in self.blocks:
            for stmt in bb.stmts:
                result_width = max(result_width, len(printer.result_str(stmt._results)))

        with printer.align(result_width):
            with printer.indent(increase=2, mark=True):
                printer.print_newline()
                for idx, bb in enumerate(self.blocks):
                    printer.print(bb)

                    if idx != len(self.blocks) - 1:
                        printer.print_newline()

        printer.print_newline()
        printer.plain_print("}")
