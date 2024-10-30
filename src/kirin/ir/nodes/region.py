from __future__ import annotations

from typing import TYPE_CHECKING, Iterable, Iterator

from typing_extensions import Self

from kirin.exceptions import VerificationError
from kirin.ir.derive import derive, field
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.block import Block
from kirin.ir.nodes.view import MutableSequenceView
from kirin.ir.ssa import SSAValue

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement
    from kirin.print import Printer


@derive(init=True, repr=True)
class RegionBlocks(MutableSequenceView[list[Block], "Region", Block]):

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

    def clone(self, ssamap: dict[SSAValue, SSAValue] | None = None) -> Region:
        """Clone a region. This will clone all blocks and statements in the region.
        `SSAValue` defined outside the region will not be cloned unless provided in `ssamap`.
        """
        ret = Region()
        successor_map: dict[Block, Block] = {}
        _ssamap = ssamap or {}
        for block in self.blocks:
            new_block = Block()
            ret.blocks.append(new_block)
            successor_map[block] = new_block
            for arg in block.args:
                new_arg = new_block.args.append_from(arg.type, arg.name)
                _ssamap[arg] = new_arg

        # update statements
        for block in self.blocks:
            for stmt in block.stmts:
                new_stmt = stmt.from_stmt(
                    stmt,
                    args=[_ssamap[arg] for arg in stmt.args],
                    regions=[region.clone(_ssamap) for region in stmt.regions],
                    successors=[
                        successor_map[successor] for successor in stmt.successors
                    ],
                )
                successor_map[block].stmts.append(new_stmt)
                for result, new_result in zip(stmt.results, new_stmt.results):
                    _ssamap[result] = new_result

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
        # populate block ids
        for block in self.blocks:
            printer.state.block_id[block]

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

    def typecheck(self) -> None:
        for block in self.blocks:
            block.typecheck()

    def verify(self) -> None:
        from kirin.ir.nodes.stmt import Statement

        if not isinstance(self.parent_node, Statement):
            raise VerificationError(
                self, "expect Region to have a parent of type Statement"
            )

        for block in self.blocks:
            block.verify()
