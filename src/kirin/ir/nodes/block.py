from __future__ import annotations

from typing import TYPE_CHECKING, Iterable, Iterator
from dataclasses import field, dataclass
from collections.abc import Sequence

from typing_extensions import Self

from kirin.print import Printer
from kirin.ir.ssa import SSAValue, BlockArgument
from kirin.exceptions import VerificationError
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.view import View, MutableSequenceView

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement
    from kirin.ir.attrs.types import TypeAttribute
    from kirin.ir.nodes.region import Region


@dataclass
class BlockArguments(MutableSequenceView[tuple, "Block", BlockArgument]):
    """A View object that contains a list of BlockArgument.

    Description:
        This is a proxy object that provide safe API to manipulate the arguments of a Block.


    """

    def append_from(self, typ: TypeAttribute, name: str | None = None) -> BlockArgument:
        """Append a new argument to the Block that this View reference to.

        Description:
            This method will create a new [`BlockArgument`][kirin.ir.BlockArgument] and append it to the argument list
            of the reference `Block`.

        Args:
            typ (TypeAttribute): The type of the argument.
            name (str | None, optional): name of the argument. Defaults to `None`.

        Returns:
            BlockArgument: The newly created [`BlockArgument`][kirin.ir.BlockArgument].

        """
        new_arg = BlockArgument(self.node, len(self.node._args), typ)
        if name:
            new_arg.name = name

        self.node._args += (new_arg,)
        return new_arg

    def insert_from(
        self, idx: int, typ: TypeAttribute, name: str | None = None
    ) -> BlockArgument:
        """Insert a new argument to the Block that this View reference to.

        Description:
            This method will create a new `BlockArgument` and insert it to the argument list
            of the reference Block at the specified index

        Args:
            idx (int): Insert location index.
            typ (TypeAttribute): The type of the argument.
            name (str | None, optional): Name of the argument. Defaults to `None`.

        Returns:
            BlockArgument: The newly created BlockArgument.
        """
        if idx < 0 or idx > len(self.node._args):
            raise ValueError("Invalid index")

        new_arg = BlockArgument(self.node, idx, typ)
        if name:
            new_arg.name = name

        for arg in self.node._args[idx:]:
            arg.index += 1
        self.node._args = self.node._args[:idx] + (new_arg,) + self.node._args[idx:]
        return new_arg

    def delete(self, arg: BlockArgument, safe: bool = True) -> None:
        """Delete a BlockArgument from the Block that this View reference to.


        Args:
            arg (BlockArgument): _description_
            safe (bool, optional): If True, error will be raised if the BlockArgument has any Use by others.  Defaults to True.

        Raises:
            ValueError: If the argument does not belong to the reference block.
        """
        if arg.block is not self.node:
            raise ValueError("Attempt to delete an argument that is not in the block")

        for block_arg in self.field[arg.index + 1 :]:
            block_arg.index -= 1
        self.node._args = (*self.field[: arg.index], *self.field[arg.index + 1 :])
        arg.delete(safe=safe)

    def __delitem__(self, idx: int) -> None:
        self.delete(self.field[idx])


@dataclass
class BlockStmtIterator:
    """Proxy object to iterate over the Statements in a Block."""

    next_stmt: Statement | None

    def __iter__(self) -> BlockStmtIterator:
        return self

    def __next__(self) -> Statement:
        if self.next_stmt is None:
            raise StopIteration
        stmt = self.next_stmt
        self.next_stmt = stmt.next_stmt
        return stmt


@dataclass
class BlockStmtsReverseIterator:
    """Proxy object to iterate over the Statements in a Block in reverse order."""

    next_stmt: Statement | None

    def __iter__(self) -> BlockStmtsReverseIterator:
        return self

    def __next__(self) -> Statement:
        if self.next_stmt is None:
            raise StopIteration
        stmt = self.next_stmt
        self.next_stmt = stmt.prev_stmt
        return stmt


@dataclass
class BlockStmts(View["Block", "Statement"]):
    """A View object that contains a list of Statements.

    Description:
        This is a proxy object that provide safe API to manipulate the statements of a Block.
    """

    def __iter__(self) -> Iterator[Statement]:
        return BlockStmtIterator(self.node.first_stmt)

    def __len__(self) -> int:
        return self.node._stmt_len

    def __reversed__(self) -> Iterator[Statement]:
        return BlockStmtsReverseIterator(self.node.last_stmt)

    def __repr__(self) -> str:
        return f"BlockStmts(len={len(self)})"

    def __getitem__(self, index: int) -> Statement:
        raise NotImplementedError("Use at() instead")

    def at(self, index: int) -> Statement:
        """This is similar to __getitem__ but due to the nature of the linked list,
        it is less efficient than __getitem__.

        Args:
            index (int): Index of the Statement.

        Returns:
            Statement: The Statement at the specified index.
        """
        if index >= len(self):
            raise IndexError("Index out of range")

        if index < 0:
            return self._at_reverse(-index - 1)

        return self._at_forward(index)

    def _at_forward(self, index: int) -> Statement:
        if self.node.first_stmt is None:
            raise IndexError("Index out of range")

        stmt = self.node.first_stmt
        for _ in range(index):
            if stmt is None:
                raise IndexError("Index out of range")
            stmt = stmt.next_stmt

        if stmt is None:
            raise IndexError("Index out of range")
        return stmt

    def _at_reverse(self, index: int) -> Statement:
        if self.node.last_stmt is None:
            raise IndexError("Index out of range")

        stmt = self.node.last_stmt
        for _ in range(index):
            if stmt is None:
                raise IndexError("Index out of range")
            stmt = stmt.prev_stmt

        if stmt is None:
            raise IndexError("Index out of range")
        return stmt

    def append(self, value: Statement) -> None:
        """Append a Statement to the reference Block.

        Args:
            value (Statement): A Statement to be appended.
        """
        from kirin.ir.nodes.stmt import Statement

        if not isinstance(value, Statement):
            raise ValueError(f"Expected Statement, got {type(value).__name__}")

        if self.node._stmt_len == 0:  # empty block
            value.attach(self.node)
            self.node._first_stmt = value
            self.node._last_stmt = value
            self.node._stmt_len += 1
        elif self.node._last_stmt:
            value.insert_after(self.node._last_stmt)
        else:
            raise ValueError("Invalid block, last_stmt is None")


@dataclass
class Block(IRNode["Region"]):
    """
    Block consist of a list of Statements and optionally input arguments.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    _args: tuple[BlockArgument, ...]

    # NOTE: we need linked list since stmts are inserted frequently
    _first_stmt: Statement | None = field(repr=False)
    _last_stmt: Statement | None = field(repr=False)
    _stmt_len: int = field(default=0, repr=False)

    parent: Region | None = field(default=None, repr=False)
    """Parent Region of the Block."""

    def __init__(
        self,
        stmts: Sequence[Statement] = (),
        argtypes: Iterable[TypeAttribute] = (),
    ):
        """
        Args:
            stmts (Sequence[Statement], optional): A list of statements. Defaults to ().
            argtypes (Iterable[TypeAttribute], optional): The type of the block arguments. Defaults to ().
        """
        super().__init__()
        self._args = tuple(
            BlockArgument(self, i, argtype) for i, argtype in enumerate(argtypes)
        )

        self._first_stmt = None
        self._last_stmt = None
        self._first_branch = None
        self._last_branch = None
        self._stmt_len = 0
        self.stmts.extend(stmts)

    @property
    def parent_stmt(self) -> Statement | None:
        """parent statement of the Block."""
        if self.parent is None:
            return None
        return self.parent.parent_node

    @property
    def parent_node(self) -> Region | None:
        """Get parent Region of the Block."""
        return self.parent

    @parent_node.setter
    def parent_node(self, parent: Region | None) -> None:
        """Set the parent Region of the Block."""
        from kirin.ir.nodes.region import Region

        self.assert_parent(Region, parent)
        self.parent = parent

    @property
    def args(self) -> BlockArguments:
        """Get the  arguments of the Block.

        Returns:
            BlockArguments: The arguments view of the Block.
        """
        return BlockArguments(self, self._args)

    @property
    def first_stmt(self) -> Statement | None:
        """Get the first Statement of the Block.

        Returns:
            Statement | None: The first Statement of the Block.
        """
        return self._first_stmt

    @property
    def last_stmt(self) -> Statement | None:
        """Get the last Statement of the Block.

        Returns:
            Statement | None: The last Statement of the Block.

        """
        return self._last_stmt

    @property
    def stmts(self) -> BlockStmts:
        """Get the list of Statements of the Block.

        Returns:
            BlockStmts: The Statements of the Block.
        """
        return BlockStmts(self)

    def drop_all_references(self) -> None:
        """Remove all the dependency that reference/uses this Block."""
        self.parent = None
        for stmt in self.stmts:
            stmt.drop_all_references()

    def detach(self) -> None:
        """Detach this Block from the IR.

        Note:
            Detach only detach the Block from the IR graph. It does not remove uses that reference the Block.
        """
        if self.parent is None:
            return

        idx = self.parent[self]
        del self.parent._blocks[idx]
        del self.parent._block_idx[self]
        for block in self.parent._blocks[idx:]:
            self.parent._block_idx[block] -= 1
        self.parent = None

    def delete(self, safe: bool = True) -> None:
        """Delete the Block completely from the IR.

        Note:
            This method will detach + remove references of the block.

        Args:
            safe (bool, optional): If True, raise error if there is anything that still reference components in the block. Defaults to True.
        """
        self.detach()
        self.drop_all_references()
        for stmt in self.stmts:
            stmt.delete(safe=safe)

    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool:
        """Check if the Block is structurally equal to another Block.

        Args:
            other (Self): The other Block to compare with.
            context (dict[IRNode  |  SSAValue, IRNode  |  SSAValue] | None, optional): A map of IRNode/SSAValue to hint that they are equivalent so the check will treat them as equivalent. Defaults to None.

        Returns:
            bool: True if the Block is structurally equal to the other Block.
        """
        if context is None:
            context = {}

        if len(self._args) != len(other._args) or len(self.stmts) != len(other.stmts):
            return False

        for arg, other_arg in zip(self._args, other._args):
            if arg.type != other_arg.type:
                return False
            context[arg] = other_arg

        context[self] = other
        if not all(
            stmt.is_structurally_equal(other_stmt, context)
            for stmt, other_stmt in zip(self.stmts, other.stmts)
        ):
            return False

        return True

    def __hash__(self) -> int:
        return id(self)

    def walk(
        self, *, reverse: bool = False, region_first: bool = False
    ) -> Iterator[Statement]:
        """Traversal the Statements in a Block.

        Args:
            reverse (bool, optional): If walk in the reversed manner. Defaults to False.
            region_first (bool, optional): If the walk should go through the Statement first or the Region of a Statement first. Defaults to False.

        Yields:
            Iterator[Statement]: An iterator that yield Statements in the Block in the specified order.
        """
        for stmt in reversed(self.stmts) if reverse else self.stmts:
            yield from stmt.walk(reverse=reverse, region_first=region_first)

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(printer.state.block_id[self])
        printer.print_seq(
            [printer.state.ssa_id[arg] for arg in self.args],
            delim=", ",
            prefix="(",
            suffix="):",
            emit=printer.plain_print,
        )

        if printer.analysis is not None:
            with printer.indent(increase=4, mark=False):
                for arg in self.args:
                    printer.print_newline()
                    with printer.rich(style="warning"):
                        printer.print_analysis(
                            arg, prefix=f"{printer.state.ssa_id[arg]} --> "
                        )

        with printer.indent(increase=2, mark=False):
            for stmt in self.stmts:
                printer.print_newline()
                printer.print_stmt(stmt)

    def typecheck(self) -> None:
        """Checking the types of the Statments in the Block."""
        for stmt in self.stmts:
            stmt.typecheck()

    def verify(self) -> None:
        """Verify the correctness of the Block.

        Raises:
            VerificationError: If the Block is not correct.
        """
        from kirin.ir.nodes.stmt import Region

        if not isinstance(self.parent, Region):
            raise VerificationError(self, "Parent is not a region")

        for stmt in self.stmts:
            stmt.verify()
