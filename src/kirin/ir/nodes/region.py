from __future__ import annotations

from typing import TYPE_CHECKING, Iterable, Iterator
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin.ir.ssa import SSAValue
from kirin.exceptions import VerificationError
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.view import MutableSequenceView
from kirin.ir.nodes.block import Block

if TYPE_CHECKING:
    from kirin.print import Printer
    from kirin.ir.nodes.stmt import Statement


@dataclass
class RegionBlocks(MutableSequenceView[list[Block], "Region", Block]):
    """A View object that contains a list of Blocks of a Region.

    Description:
        This is a proxy object that provide safe API to manipulate the Blocks of a Region.

    """

    def __setitem__(
        self, idx: int | slice, block_or_blocks: Block | Iterable[Block]
    ) -> None:
        """Replace/Set the Blocks of the Region.

        Args:
            idx (int | slice): The index or slice to replace the [`Blocks`][kirin.ir.Block].
            block_or_blocks (Block | Iterable[Block]): The Block or Blocks to replace the Blocks.

        """
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
        self.field[idx].delete()

    def pop(self, idx: int = -1) -> Block:
        item = self.field[idx]
        self[idx].detach()
        return item

    def insert(self, idx: int, value: Block) -> None:
        """Inserts a Block at the specified index.

        Args:
            idx (int): The index at which to insert the block.
            value (Block): The block to be inserted.
        """
        value.attach(self.node)
        self.field.insert(idx, value)
        for i, value in enumerate(self.field[idx:], idx):
            self.node._block_idx[value] = i

    def append(self, value: Block) -> None:
        """Append a Block to the Region.

        Args:
            value (Block): The block to be appended.
        """
        value.attach(self.node)
        self.node._block_idx[value] = len(self.field)
        self.field.append(value)


@dataclass
class Region(IRNode["Statement"]):
    """Region consist of a list of Blocks

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    _blocks: list[Block] = field(default_factory=list, repr=False)
    _block_idx: dict[Block, int] = field(default_factory=dict, repr=False)
    _parent: Statement | None = field(default=None, repr=False)

    def __init__(
        self,
        blocks: Block | Iterable[Block] = (),
        parent: Statement | None = None,
    ):
        """Initialize a Region object.

        Args:
            blocks (Block | Iterable[Block], optional): A single [`Block`][kirin.ir.Block] object or an iterable of Block objects. Defaults to ().
            parent (Statement | None, optional): The parent [`Statement`][kirin.ir.Statement] object. Defaults to None.
        """
        self._blocks = []
        self._block_idx = {}
        self.parent_node = parent
        if isinstance(blocks, Block):
            blocks = (blocks,)
        for block in blocks:
            self.blocks.append(block)

    def __getitem__(self, block: Block) -> int:
        """Get the index of a block within the region.

        Args:
            block (Block): The block to get the index of.

        Raises:
            ValueError: If the block does not belong to the region.

        Returns:
            int: The index of the block within the region.
        """
        if block.parent is not self:
            raise ValueError("Block does not belong to the region")
        return self._block_idx[block]

    def __hash__(self) -> int:
        return id(self)

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
                    args=[_ssamap.get(arg, arg) for arg in stmt.args],
                    regions=[region.clone(_ssamap) for region in stmt.regions],
                    successors=[
                        successor_map[successor] for successor in stmt.successors
                    ],
                )
                successor_map[block].stmts.append(new_stmt)
                for result, new_result in zip(stmt.results, new_stmt.results):
                    _ssamap[result] = new_result
                    new_result.name = result.name

        return ret

    @property
    def parent_node(self) -> Statement | None:
        """Get the parent statement of the region."""
        return self._parent

    @parent_node.setter
    def parent_node(self, parent: Statement | None) -> None:
        """Set the parent statement of the region."""
        from kirin.ir.nodes.stmt import Statement

        self.assert_parent(Statement, parent)
        self._parent = parent

    @property
    def blocks(self) -> RegionBlocks:
        """Get the Blocks in the region.

        Returns:
            RegionBlocks: The blocks View object of the region.
        """
        return RegionBlocks(self, self._blocks)

    @property
    def region_index(self) -> int:
        """Get the index of the region within the parent scope.

        Returns:
            int: The index of the region within the parent scope.
        """
        if self.parent_node is None:
            raise ValueError("Region has no parent")
        for idx, region in enumerate(self.parent_node.regions):
            if region is self:
                return idx
        raise ValueError("Region not found in parent")

    def detach(self, index: int | None = None) -> None:
        """Detach this Region from the IR tree graph.

        Note:
            Detach only detach the Region from the IR graph. It does not remove uses that reference the Region.
        """
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
        """Remove all the dependency that reference/uses this Region."""
        self.parent_node = None
        for block in self._blocks:
            block.drop_all_references()

    def delete(self, safe: bool = True) -> None:
        """Delete the Region completely from the IR graph.

        Note:
            This method will detach + remove references of the Region.

        Args:
            safe (bool, optional): If True, raise error if there is anything that still reference components in the Region. Defaults to True.
        """
        self.detach()
        self.drop_all_references()

    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool:
        """Check if the Region is structurally equal to another Region.

        Args:
            other (Self): The other Region to compare with.
            context (dict[IRNode  |  SSAValue, IRNode  |  SSAValue] | None, optional): A map of IRNode/SSAValue to hint that they are equivalent so the check will treat them as equivalent. Defaults to None.

        Returns:
            bool: True if the Region is structurally equal to the other Region.
        """
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
        """Traversal the Statements of Blocks in the Region.

        Args:
            reverse (bool, optional): If walk in the reversed manner. Defaults to False.
            region_first (bool, optional): If the walk should go through the Statement first or the Region of a Statement first. Defaults to False.

        Yields:
            Iterator[Statement]: An iterator that yield Statements of Blocks in the Region, in the specified order.
        """
        for block in reversed(self.blocks) if reverse else self.blocks:
            yield from block.walk(reverse=reverse, region_first=region_first)

    def stmts(self) -> Iterator[Statement]:
        """Iterate over all the Statements in the Region. This does not walk into nested Regions.

        Yields:
            Iterator[Statement]: An iterator that yield Statements of Blocks in the Region.
        """
        for block in self.blocks:
            yield from block.stmts

    def print_impl(self, printer: Printer) -> None:
        # populate block ids
        for block in self.blocks:
            printer.state.block_id[block]

        printer.plain_print("{")
        if len(self.blocks) == 0:
            printer.print_newline()
            printer.plain_print("}")
            return

        with printer.align(printer.result_width(self.stmts())):
            with printer.indent(increase=2, mark=True):
                printer.print_newline()
                for idx, bb in enumerate(self.blocks):
                    printer.print(bb)

                    if idx != len(self.blocks) - 1:
                        printer.print_newline()

        printer.print_newline()
        printer.plain_print("}")

    def typecheck(self) -> None:
        """Checking the types of the Statments of Blocks in the Region."""
        for block in self.blocks:
            block.typecheck()

    def verify(self) -> None:
        """Verify the correctness of the Region.

        Raises:
            VerificationError: If the Region is not correct.
        """
        from kirin.ir.nodes.stmt import Statement

        if not isinstance(self.parent_node, Statement):
            raise VerificationError(
                self, "expect Region to have a parent of type Statement"
            )

        for block in self.blocks:
            block.verify()
