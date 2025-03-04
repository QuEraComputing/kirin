from __future__ import annotations

from typing import TYPE_CHECKING, Mapping, TypeVar, ClassVar, Iterator, Sequence
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin.print import Printer, Printable
from kirin.ir.ssa import SSAValue, ResultValue
from kirin.ir.use import Use
from kirin.ir.traits import StmtTrait
from kirin.ir.attrs.abc import Attribute
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.view import MutableSequenceView
from kirin.ir.nodes.block import Block
from kirin.ir.nodes.region import Region

if TYPE_CHECKING:
    from kirin.source import SourceInfo
    from kirin.ir.dialect import Dialect
    from kirin.ir.attrs.types import TypeAttribute
    from kirin.ir.nodes.block import Block
    from kirin.ir.nodes.region import Region


@dataclass
class ArgumentList(
    MutableSequenceView[tuple[SSAValue, ...], "Statement", SSAValue], Printable
):
    """A View object that contains a list of Arguemnts of a Statement.

    Description:
        This is a proxy object that provide safe API to manipulate the arguemnts of a statement.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    def __delitem__(self, idx: int) -> None:
        arg = self.field[idx]
        arg.remove_use(Use(self.node, idx))
        new_args = (*self.field[:idx], *self.field[idx + 1 :])
        self.node._args = new_args
        self.field = new_args

    def set_item(self, idx: int, value: SSAValue) -> None:
        """Set the argument SSAVAlue at the specified index.

        Args:
            idx (int): The index of the item to set.
            value (SSAValue): The value to set.
        """
        args = self.field
        args[idx].remove_use(Use(self.node, idx))
        value.add_use(Use(self.node, idx))
        new_args = (*args[:idx], value, *args[idx + 1 :])
        self.node._args = new_args
        self.field = new_args

    def insert(self, idx: int, value: SSAValue) -> None:
        """Insert the argument SSAValue at the specified index.

        Args:
            idx (int): The index to insert the value.
            value (SSAValue): The value to insert.
        """
        args = self.field
        value.add_use(Use(self.node, idx))
        new_args = (*args[:idx], value, *args[idx:])
        self.node._args = new_args
        self.field = new_args

    def get_slice(self, name: str) -> slice:
        """Get the slice of the arguments.

        Args:
            name (str): The name of the slice.

        Returns:
            slice: The slice of the arguments.
        """
        index = self.node._name_args_slice[name]
        if isinstance(index, int):
            return slice(index, index + 1)
        return index

    def print_impl(self, printer: Printer) -> None:
        printer.print_seq(self.field, delim=", ", prefix="[", suffix="]")


@dataclass
class ResultList(MutableSequenceView[list[ResultValue], "Statement", ResultValue]):
    """A View object that contains a list of ResultValue of a Statement.

    Description:
        This is a proxy object that provide safe API to manipulate the result values of a statement

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    def __setitem__(
        self, idx: int | slice, value: ResultValue | Sequence[ResultValue]
    ) -> None:
        raise NotImplementedError("Cannot set result value directly")

    def __delitem__(self, idx: int) -> None:
        result = self.field[idx]
        del self.field[idx]
        result.delete()

    @property
    def types(self) -> Sequence[TypeAttribute]:
        """Get the result types of the Statement.

        Returns:
            Sequence[TypeAttribute]: type of each result value.
        """
        return [result.type for result in self.field]


@dataclass(repr=False)
class Statement(IRNode["Block"]):
    """The Statment is an instruction in the IR

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    name: ClassVar[str]
    dialect: ClassVar[Dialect | None] = field(default=None, init=False, repr=False)
    traits: ClassVar[frozenset[StmtTrait]]
    _arg_groups: ClassVar[frozenset[str]] = frozenset()

    _args: tuple[SSAValue, ...] = field(init=False)
    _results: list[ResultValue] = field(init=False, default_factory=list)
    successors: list[Block] = field(init=False)
    _regions: list[Region] = field(init=False)
    attributes: dict[str, Attribute] = field(init=False)

    parent: Block | None = field(default=None, init=False, repr=False)
    _next_stmt: Statement | None = field(default=None, init=False, repr=False)
    _prev_stmt: Statement | None = field(default=None, init=False, repr=False)

    # NOTE: This is only for syntax sugar to provide
    # access to args via the properties
    _name_args_slice: dict[str, int | slice] = field(
        init=False, repr=False, default_factory=dict
    )
    source: SourceInfo | None = field(default=None, init=False, repr=False)

    @property
    def parent_stmt(self) -> Statement | None:
        """Get the parent statement.

        Returns:
            Statement | None: The parent statement.
        """
        if not self.parent_node:
            return None
        return self.parent_node.parent_stmt

    @property
    def parent_node(self) -> Block | None:
        """Get the parent node.

        Returns:
            Block | None: The parent node.
        """
        return self.parent

    @parent_node.setter
    def parent_node(self, parent: Block | None) -> None:
        """Set the parent Block."""
        from kirin.ir.nodes.block import Block

        self.assert_parent(Block, parent)
        self.parent = parent

    @property
    def parent_region(self) -> Region | None:
        """Get the parent Region.
        Returns:
            Region | None: The parent Region.
        """
        if (p := self.parent_node) is not None:
            return p.parent_node
        return None

    @property
    def parent_block(self) -> Block | None:
        """Get the parent Block.

        Returns:
            Block | None: The parent Block.
        """
        return self.parent_node

    @property
    def next_stmt(self) -> Statement | None:
        """Get the next statement."""
        return self._next_stmt

    @next_stmt.setter
    def next_stmt(self, stmt: Statement) -> None:
        """Set the next statement.

        Note:
            Do not directly call this API. use `stmt.insert_after(self)` instead.

        """
        raise ValueError(
            "Cannot set next_stmt directly, use stmt.insert_after(self) or stmt.insert_before(self)"
        )

    @property
    def prev_stmt(self) -> Statement | None:
        """Get the previous statement."""
        return self._prev_stmt

    @prev_stmt.setter
    def prev_stmt(self, stmt: Statement) -> None:
        """Set the previous statement.

        Note:
            Do not directly call this API. use `stmt.insert_before(self)` instead

        """
        raise ValueError(
            "Cannot set prev_stmt directly, use stmt.insert_after(self) or stmt.insert_before(self)"
        )

    def insert_after(self, stmt: Statement) -> None:
        """Insert the current Statement after the input Statement.

        Args:
            stmt (Statement): Input Statement.

        Example:
            The following example demonstrates how to insert a Statement after another Statement.
            After `insert_after` is called, `stmt1` will be inserted after `stmt2`, which appears in IR in the order (stmt2 -> stmt1)
            ```python
            stmt1 = Statement()
            stmt2 = Statement()
            stmt1.insert_after(stmt2)
            ```
        """
        if self._next_stmt is not None and self._prev_stmt is not None:
            raise ValueError(
                f"Cannot insert before a statement that is already in a block: {self.name}"
            )

        if stmt._next_stmt is not None:
            stmt._next_stmt._prev_stmt = self

        self._prev_stmt = stmt
        self._next_stmt = stmt._next_stmt

        self.parent = stmt.parent
        stmt._next_stmt = self

        if self.parent:
            self.parent._stmt_len += 1

            if self._next_stmt is None:
                self.parent._last_stmt = self

    def insert_before(self, stmt: Statement) -> None:
        """Insert the current Statement before the input Statement.

        Args:
            stmt (Statement): Input Statement.

        Example:
            The following example demonstrates how to insert a Statement before another Statement.
            After `insert_before` is called, `stmt1` will be inserted before `stmt2`, which appears in IR in the order (stmt1 -> stmt2)
            ```python
            stmt1 = Statement()
            stmt2 = Statement()
            stmt1.insert_before(stmt2)
            ```
        """
        if self._next_stmt is not None and self._prev_stmt is not None:
            raise ValueError(
                f"Cannot insert before a statement that is already in a block: {self.name}"
            )

        if stmt._prev_stmt is not None:
            stmt._prev_stmt._next_stmt = self

        self._next_stmt = stmt
        self._prev_stmt = stmt._prev_stmt

        self.parent = stmt.parent
        stmt._prev_stmt = self

        if self.parent:
            self.parent._stmt_len += 1

            if self._prev_stmt is None:
                self.parent._first_stmt = self

    def replace_by(self, stmt: Statement) -> None:
        """Replace the current Statement by the input Statement.

        Args:
            stmt (Statement): Input Statement.
        """
        stmt.insert_before(self)
        for result, old_result in zip(stmt._results, self._results):
            old_result.replace_by(result)
            if old_result.name:
                result.name = old_result.name
        self.delete()

    @property
    def args(self) -> ArgumentList:
        """Get the arguments of the Statement.

        Returns:
            ArgumentList: The arguments View of the Statement.
        """
        return ArgumentList(self, self._args)

    @args.setter
    def args(self, args: Sequence[SSAValue]) -> None:
        """Set the arguments of the Statement.

        Args:
            args (Sequence[SSAValue]): The arguments to set.
        """
        new = tuple(args)
        for idx, arg in enumerate(self._args):
            arg.remove_use(Use(self, idx))
        for idx, arg in enumerate(new):
            arg.add_use(Use(self, idx))
        self._args = new

    @property
    def results(self) -> ResultList:
        """Get the result values of the Statement.

        Returns:
            ResultList: The result values View of the Statement.
        """
        return ResultList(self, self._results)

    @property
    def regions(self) -> list[Region]:
        """Get a list of regions of the Statement.

        Returns:
            list[Region]: The list of regions of the Statement.
        """
        return self._regions

    @regions.setter
    def regions(self, regions: list[Region]) -> None:
        """Set the regions of the Statement."""
        for region in self._regions:
            region._parent = None
        for region in regions:
            region._parent = self
        self._regions = regions

    def drop_all_references(self) -> None:
        """Remove all the dependency that reference/uses this Statement."""
        self.parent = None
        for idx, arg in enumerate(self._args):
            arg.remove_use(Use(self, idx))
        for region in self._regions:
            region.drop_all_references()

    def delete(self, safe: bool = True) -> None:
        """Delete the Statement completely from the IR graph.

        Note:
            This method will detach + remove references of the Statement.

        Args:
            safe (bool, optional): If True, raise error if there is anything that still reference components in the Statement. Defaults to True.
        """
        self.detach()
        self.drop_all_references()
        for result in self._results:
            result.delete(safe=safe)

    def detach(self) -> None:
        """detach the statement from its parent block."""
        if self.parent is None:
            return

        parent: Block = self.parent
        prev_stmt = self.prev_stmt
        next_stmt = self.next_stmt

        if prev_stmt is not None:
            prev_stmt._next_stmt = next_stmt
            self._prev_stmt = None
        else:
            assert (
                parent._first_stmt is self
            ), "Invalid statement, has no prev_stmt but not first_stmt"
            parent._first_stmt = next_stmt

        if next_stmt is not None:
            next_stmt._prev_stmt = prev_stmt
            self._next_stmt = None
        else:
            assert (
                parent._last_stmt is self
            ), "Invalid statement, has no next_stmt but not last_stmt"
            parent._last_stmt = prev_stmt

        self.parent = None
        parent._stmt_len -= 1
        return

    def __post_init__(self):
        assert self.name != ""
        assert isinstance(self.name, str)

    def __init__(
        self,
        *,
        args: Sequence[SSAValue] = (),
        regions: Sequence[Region] = (),
        successors: Sequence[Block] = (),
        attributes: Mapping[str, Attribute] = {},
        results: Sequence[ResultValue] = (),
        result_types: Sequence[TypeAttribute] = (),
        args_slice: Mapping[str, int | slice] = {},
        source: SourceInfo | None = None,
    ) -> None:
        super().__init__()
        """Initialize the Statement.

        Args:
            arsg (Sequence[SSAValue], optional): The arguments of the Statement. Defaults to ().
            regions (Sequence[Region], optional): The regions where the Statement belong to. Defaults to ().
            successors (Sequence[Block], optional): The successors of the Statement. Defaults to ().
            attributes (Mapping[str, Attribute], optional): The attributes of the Statement. Defaults to {}.
            results (Sequence[ResultValue], optional): The result values of the Statement. Defaults to ().
            result_types (Sequence[TypeAttribute], optional): The result types of the Statement. Defaults to ().
            args_slice (Mapping[str, int | slice], optional): The arguments slice of the Statement. Defaults to {}.
            source (SourceInfo | None, optional): The source information of the Statement for debugging/stacktracing. Defaults to None.

        """
        self._args = ()
        self._regions = []
        self._name_args_slice = dict(args_slice)
        self.source = source
        self.args = args

        if results:
            self._results = list(results)
            assert (
                len(result_types) == 0
            ), "expect either results or result_types specified, got both"

        if result_types:
            self._results = [
                ResultValue(self, idx, type=type)
                for idx, type in enumerate(result_types)
            ]

        if not results and not result_types:
            self._results = list(results)

        self.successors = list(successors)
        self.attributes = dict(attributes)
        self.regions = list(regions)

        self.parent = None
        self._next_stmt = None
        self._prev_stmt = None
        self.__post_init__()

    @classmethod
    def from_stmt(
        cls,
        other: Statement,
        args: Sequence[SSAValue] | None = None,
        regions: list[Region] | None = None,
        successors: list[Block] | None = None,
        attributes: dict[str, Attribute] | None = None,
    ) -> Self:
        """Create a similar Statement with new `ResultValue` and without
        attaching to any parent block. This still references to the old successor
        and regions.
        """
        obj = cls.__new__(cls)
        Statement.__init__(
            obj,
            args=args or other._args,
            regions=regions or other._regions,
            successors=successors or other.successors,
            attributes=attributes or other.attributes,
            result_types=[result.type for result in other._results],
            args_slice=other._name_args_slice,
        )
        # inherit the hint:
        for result, other_result in zip(obj._results, other._results):
            result.hints = dict(other_result.hints)

        return obj

    def walk(
        self,
        *,
        reverse: bool = False,
        region_first: bool = False,
        include_self: bool = True,
    ) -> Iterator[Statement]:
        """Traversal the Statements of Regions.

        Args:
            reverse (bool, optional): If walk in the reversed manner. Defaults to False.
            region_first (bool, optional): If the walk should go through the Statement first or the Region of a Statement first. Defaults to False.
            include_self (bool, optional): If the walk should include the Statement itself. Defaults to True.

        Yields:
            Iterator[Statement]: An iterator that yield Statements of Blocks in the Region, in the specified order.
        """
        if include_self and not region_first:
            yield self

        for region in reversed(self.regions) if reverse else self.regions:
            yield from region.walk(reverse=reverse, region_first=region_first)

        if include_self and region_first:
            yield self

    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool:
        """Check if the Statement is structurally equal to another Statement.

        Args:
            other (Self): The other Statelemt to compare with.
            context (dict[IRNode  |  SSAValue, IRNode  |  SSAValue] | None, optional): A map of IRNode/SSAValue to hint that they are equivalent so the check will treat them as equivalent. Defaults to None.

        Returns:
            bool: True if the IRNode is structurally equal to the other.
        """
        if context is None:
            context = {}

        if self.name != other.name:
            return False

        if (
            len(self.args) != len(other.args)
            or len(self.regions) != len(other.regions)
            or len(self.successors) != len(other.successors)
            or self.attributes != other.attributes
        ):
            return False

        if (
            self.parent is not None
            and other.parent is not None
            and context.get(self.parent) != other.parent
        ):
            return False

        if not all(
            context.get(arg, arg) == other_arg
            for arg, other_arg in zip(self.args, other.args)
        ):
            return False

        if not all(
            context.get(successor, successor) == other_successor
            for successor, other_successor in zip(self.successors, other.successors)
        ):
            return False

        if not all(
            region.is_structurally_equal(other_region, context)
            for region, other_region in zip(self.regions, other.regions)
        ):
            return False

        for result, other_result in zip(self._results, other._results):
            context[result] = other_result

        return True

    def __hash__(self) -> int:
        return id(self)

    def print_impl(self, printer: Printer) -> None:
        from kirin.decl import fields as stmt_fields

        printer.print_name(self)
        printer.plain_print("(")
        for idx, (name, s) in enumerate(self._name_args_slice.items()):
            values = self.args[s]
            if (fields := stmt_fields(self)) and not fields.args[name].print:
                pass
            else:
                with printer.rich(style="orange4"):
                    printer.plain_print(name, "=")

            if isinstance(values, SSAValue):
                printer.print(values)
            else:
                printer.print_seq(values, delim=", ", prefix="(", suffix=")")

            if idx < len(self._name_args_slice) - 1:
                printer.plain_print(", ")

        # NOTE: args are specified manually without names
        if not self._name_args_slice and self._args:
            printer.print_seq(self._args, delim=", ")

        printer.plain_print(")")

        if self.successors:
            printer.print_seq(
                (printer.state.block_id[successor] for successor in self.successors),
                emit=printer.plain_print,
                delim=", ",
                prefix="[",
                suffix="]",
            )

        if self.regions:
            printer.print_seq(
                self.regions,
                delim=" ",
                prefix=" (",
                suffix=")",
            )

        if self.attributes:
            printer.plain_print("{")
            with printer.rich(highlight=True):
                printer.print_mapping(self.attributes, delim=", ")
            printer.plain_print("}")

        if self._results:
            with printer.rich(style="black"):
                printer.plain_print(" : ")
                printer.print_seq(
                    [result.type for result in self._results],
                    delim=", ",
                )

    def get_attr_or_prop(self, key: str) -> Attribute | None:
        """Get the attribute or property of the Statement.

        Args:
            key (str): The key of the attribute or property.

        Returns:
            Attribute | None: The attribute or property of the Statement.
        """
        return self.attributes.get(key)

    @classmethod
    def has_trait(cls, trait_type: type[StmtTrait]) -> bool:
        """Check if the Statement has a specific trait.

        Args:
            trait_type (type[StmtTrait]): The type of trait to check for.

        Returns:
            bool: True if the class has the specified trait, False otherwise.
        """
        for trait in cls.traits:
            if isinstance(trait, trait_type):
                return True
        return False

    TraitType = TypeVar("TraitType", bound=StmtTrait)

    @classmethod
    def get_trait(cls, trait: type[TraitType]) -> TraitType | None:
        """Get the trait of the Statement."""
        for t in cls.traits:
            if isinstance(t, trait):
                return t
        return None

    def expect_one_result(self) -> ResultValue:
        """Check if the statement contain only one result, and return it"""
        if len(self._results) != 1:
            raise ValueError(f"expected one result, got {len(self._results)}")
        return self._results[0]

    # NOTE: statement should implement typecheck
    # this is done automatically via @statement, but
    # in the case manualy implementation is needed,
    # it should be implemented here.
    # NOTE: not an @abstractmethod to make linter happy
    def typecheck(self) -> None:
        """check the type of the statement.

        Note:
            1. Statement should implement typecheck.
            this is done automatically via @statement, but
            in the case manualy implementation is needed,
            it should be implemented here.
            2. This API should be called after all the types are figured out (by typeinfer)

        """
        raise NotImplementedError

    def verify(self) -> None:
        return
