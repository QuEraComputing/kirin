from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Iterable, Iterator

from typing_extensions import Self

from kirin.ir.attrs import TypeAttribute
from kirin.ir.derive import derive, field
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.view import MutableSequenceView, View
from kirin.ir.ssa import BlockArgument, SSAValue
from kirin.print import Printer

if TYPE_CHECKING:
    from kirin.ir.nodes.region import Region
    from kirin.ir.nodes.stmt import Statement


@derive(init=True, repr=True)
class BlockArguments(MutableSequenceView[tuple, "Block", BlockArgument]):

    def append_from(self, typ: TypeAttribute, name: str | None = None) -> BlockArgument:
        new_arg = BlockArgument(self.node, len(self.node._args), typ)
        if name:
            new_arg.name = name

        self.node._args += (new_arg,)
        return new_arg

    def insert_from(
        self, idx: int, typ: TypeAttribute, name: str | None = None
    ) -> BlockArgument:
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
        if arg.block is not self.node:
            raise ValueError("Attempt to delete an argument that is not in the block")

        for block_arg in self.field[arg.index + 1 :]:
            block_arg.index -= 1
        self.node._args = (*self.field[: arg.index], *self.field[arg.index + 1 :])
        arg.delete(safe=safe)

    def __delitem__(self, idx: int) -> None:
        self.delete(self.field[idx])


@derive(init=True, repr=True)
class BlockStmtIterator:
    next_stmt: Statement | None

    def __iter__(self) -> BlockStmtIterator:
        return self

    def __next__(self) -> Statement:
        if self.next_stmt is None:
            raise StopIteration
        stmt = self.next_stmt
        self.next_stmt = stmt.next_stmt
        return stmt


@derive(init=True, repr=True)
class BlockStmtsReverseIterator:
    next_stmt: Statement | None

    def __iter__(self) -> BlockStmtsReverseIterator:
        return self

    def __next__(self) -> Statement:
        if self.next_stmt is None:
            raise StopIteration
        stmt = self.next_stmt
        self.next_stmt = stmt.prev_stmt
        return stmt


@derive(init=True, repr=True)
class BlockStmts(View["Block", "Statement"]):
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
        """
        if index < 0 or index >= len(self):
            raise IndexError("Index out of range")

        # NOTE: we checked the length, so we are sure
        # that first_stmt is not None
        stmt: Statement = self.node.first_stmt  # type: ignore
        for _ in range(index):
            stmt = stmt.next_stmt  # type: ignore
        return stmt

    def append(self, value: Statement) -> None:
        from kirin.ir.nodes.stmt import Statement

        if not isinstance(value, Statement):
            raise ValueError(f"Expected Statement, got {type(value).__name__}")

        if self.node._stmt_len == 0:  # empty block
            value.attach(self.node)
            self.node._first_stmt = value
            self.node._last_stmt = value
        elif self.node._last_stmt:
            value.insert_after(self.node._last_stmt)
        else:
            raise ValueError("Invalid block, last_stmt is None")
        self.node._stmt_len += 1


@derive(id_hash=True)
class Block(IRNode["Region"]):
    _args: tuple[BlockArgument, ...]

    # NOTE: we need linked list since stmts are inserted frequently
    _first_stmt: Statement | None = field(repr=False)
    _last_stmt: Statement | None = field(repr=False)
    _stmt_len: int = field(default=0, repr=False)

    parent: Region | None = field(default=None, repr=False)

    def __init__(
        self,
        stmts: Sequence[Statement] = (),
        argtypes: Iterable[TypeAttribute] = (),
    ):
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
        if self.parent is None:
            return None
        return self.parent.parent_node

    @property
    def parent_node(self) -> Region | None:
        return self.parent

    @parent_node.setter
    def parent_node(self, parent: Region | None) -> None:
        from kirin.ir.nodes.region import Region

        self.assert_parent(Region, parent)
        self.parent = parent

    @property
    def args(self) -> BlockArguments:
        return BlockArguments(self, self._args)

    @property
    def first_stmt(self) -> Statement | None:
        return self._first_stmt

    @property
    def last_stmt(self) -> Statement | None:
        return self._last_stmt

    @property
    def stmts(self) -> BlockStmts:
        return BlockStmts(self)

    def clone(self) -> Block:
        ret = Block()
        ssamap: dict[SSAValue, SSAValue] = {}
        for arg in self.args:
            new_arg = ret.args.append_from(arg.type, arg.name)
            ssamap[arg] = new_arg

        # NOTE: arg is always in ssamap, otherwise this implementation is incorrect
        stmt = self.first_stmt
        while stmt is not None:
            # leave the successors if there are any, cuz we may just change that later
            new_stmt = stmt.from_stmt(
                stmt,
                args=[ssamap.get(arg, arg) for arg in stmt.args],
                attributes=stmt.attributes.copy(),  # copy so that we owns it
                regions=[
                    region.clone() for region in stmt.regions
                ],  # clone the regions, because stmt owns it
            )
            ret.stmts.append(new_stmt)
            for result, new_result in zip(stmt.results, new_stmt.results):
                ssamap[result] = new_result

            stmt = stmt.next_stmt

        return ret

    def drop_all_references(self) -> None:
        self.parent = None
        for stmt in self.stmts:
            stmt.drop_all_references()

    def detach(self) -> None:
        if self.parent is None:
            return

        idx = self.parent[self]
        del self.parent._blocks[idx]
        del self.parent._block_idx[self]
        for block in self.parent._blocks[idx:]:
            self.parent._block_idx[block] -= 1
        self.parent = None

    def delete(self, safe: bool = True) -> None:
        self.detach()
        self.drop_all_references()
        for stmt in self.stmts:
            stmt.delete(safe=safe)

    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool:
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

        with printer.indent(increase=2, mark=False):
            for stmt in self.stmts:
                printer.print_newline()
                if stmt._results:
                    result_str = printer.result_str(stmt._results)
                    printer.plain_print(
                        result_str.rjust(printer.state.result_width), " = "
                    )
                elif printer.state.result_width:
                    printer.plain_print(" " * printer.state.result_width, "   ")
                with printer.indent(printer.state.result_width + 3, mark=True):
                    printer.print(stmt)
