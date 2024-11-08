from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, ClassVar, Iterator, Mapping, Sequence, TypeVar

from typing_extensions import Self

from kirin.ir.attrs import Attribute, TypeAttribute
from kirin.ir.nodes.base import IRNode
from kirin.ir.nodes.block import Block
from kirin.ir.nodes.region import Region
from kirin.ir.nodes.view import MutableSequenceView
from kirin.ir.ssa import ResultValue, SSAValue
from kirin.ir.traits import StmtTrait
from kirin.ir.use import Use
from kirin.print import Printable, Printer

if TYPE_CHECKING:
    from kirin.ir.dialect import Dialect
    from kirin.ir.nodes.block import Block
    from kirin.ir.nodes.region import Region
    from kirin.lowering import LoweringState, Result
    from kirin.source import SourceInfo


@dataclass
class ArgumentList(MutableSequenceView[tuple, "Statement", SSAValue], Printable):
    def set_item(self, idx: int, value: SSAValue) -> None:
        args = self.field
        args[idx].remove_use(Use(self.node, idx))
        value.add_use(Use(self.node, idx))
        new_args = (*args[:idx], value, *args[idx + 1 :])
        self.node._args = new_args
        self.field = new_args

    def insert(self, idx: int, value: SSAValue) -> None:
        args = self.field
        value.add_use(Use(self.node, idx))
        new_args = (*args[:idx], value, *args[idx:])
        self.node._args = new_args
        self.field = new_args

    def print_impl(self, printer: Printer) -> None:
        printer.print_seq(self.field, delim=", ", prefix="[", suffix="]")


@dataclass
class ResultList(MutableSequenceView[list, "Statement", ResultValue]):

    def __setitem__(
        self, idx: int | slice, value: ResultValue | Sequence[ResultValue]
    ) -> None:
        raise NotImplementedError("Cannot set result value directly")

    @property
    def types(self) -> Sequence[TypeAttribute]:
        return [result.type for result in self.field]


@dataclass(repr=False)
class Statement(IRNode["Block"]):
    name: ClassVar[str]
    dialect: ClassVar[Dialect | None] = field(default=None, init=False, repr=False)
    traits: ClassVar[frozenset[StmtTrait]]
    _arg_groups: ClassVar[frozenset[str]] = frozenset()

    _args: tuple[SSAValue, ...] = field(init=False)
    _results: list[ResultValue] = field(init=False, default_factory=list)
    successors: list[Block] = field(init=False)
    _regions: list[Region] = field(init=False)
    attributes: dict[str, Attribute] = field(init=False)
    properties: dict[str, Attribute] = field(init=False)

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
        if not self.parent_node:
            return None
        return self.parent_node.parent_stmt

    @property
    def parent_node(self) -> Block | None:
        return self.parent

    @parent_node.setter
    def parent_node(self, parent: Block | None) -> None:
        from kirin.ir.nodes.block import Block

        self.assert_parent(Block, parent)
        self.parent = parent

    @property
    def parent_region(self) -> Region | None:
        if (p := self.parent_node) is not None:
            return p.parent_node
        return None

    @property
    def parent_block(self) -> Block | None:
        return self.parent_node

    @property
    def next_stmt(self) -> Statement | None:
        return self._next_stmt

    @next_stmt.setter
    def next_stmt(self, stmt: Statement) -> None:
        raise ValueError(
            "Cannot set next_stmt directly, use stmt.insert_after(self) or stmt.insert_before(self)"
        )

    @property
    def prev_stmt(self) -> Statement | None:
        return self._prev_stmt

    @prev_stmt.setter
    def prev_stmt(self, stmt: Statement) -> None:
        raise ValueError(
            "Cannot set prev_stmt directly, use stmt.insert_after(self) or stmt.insert_before(self)"
        )

    def insert_after(self, stmt: Statement) -> None:
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
        stmt.insert_before(self)
        for result, old_result in zip(stmt._results, self._results):
            old_result.replace_by(result)
            if old_result.name:
                result.name = old_result.name
        self.delete()

    @property
    def args(self) -> ArgumentList:
        return ArgumentList(self, self._args)

    @args.setter
    def args(self, args: Sequence[SSAValue]) -> None:
        new = tuple(args)
        for idx, arg in enumerate(self._args):
            arg.remove_use(Use(self, idx))
        for idx, arg in enumerate(new):
            arg.add_use(Use(self, idx))
        self._args = new

    @property
    def results(self) -> ResultList:
        return ResultList(self, self._results)

    @property
    def regions(self) -> list[Region]:
        return self._regions

    @regions.setter
    def regions(self, regions: list[Region]) -> None:
        for region in self._regions:
            region._parent = None
        for region in regions:
            region._parent = self
        self._regions = regions

    def drop_all_references(self) -> None:
        self.parent = None
        for idx, arg in enumerate(self._args):
            arg.remove_use(Use(self, idx))
        for region in self._regions:
            region.drop_all_references()

    def delete(self, safe: bool = True) -> None:
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

        for key in self.attributes:
            if key in self.properties:
                raise ValueError(f"name clash: Attribute {key} is already a property")

    def __init__(
        self,
        *,
        args: Sequence[SSAValue] = (),
        regions: Sequence[Region] = (),
        successors: Sequence[Block] = (),
        attributes: Mapping[str, Attribute] = {},
        properties: Mapping[str, Attribute] = {},
        results: Sequence[ResultValue] = (),
        result_types: Sequence[TypeAttribute] = (),
        args_slice: Mapping[str, int | slice] = {},
        source: SourceInfo | None = None,
    ) -> None:
        super().__init__()

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
        self.properties = dict(properties)
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
            properties=other.properties,  # properties are immutable, thus no need to copy
            result_types=[result.type for result in other._results],
            args_slice=other._name_args_slice,
        )
        return obj

    def walk(
        self,
        *,
        reverse: bool = False,
        region_first: bool = False,
        include_self: bool = True,
    ) -> Iterator[Statement]:
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
        if context is None:
            context = {}

        if self.name != other.name:
            return False

        if (
            len(self.args) != len(other.args)
            or len(self.regions) != len(other.regions)
            or len(self.successors) != len(other.successors)
            or self.attributes != other.attributes
            or self.properties != other.properties
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
                printer.print_seq(values, delim=", ")

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

        if self.properties:
            printer.plain_print("<{")
            with printer.rich(highlight=True):
                printer.print_mapping(self.properties, delim=", ")
            printer.plain_print("}>")

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
        return self.attributes.get(key, self.properties.get(key))

    @classmethod
    def has_trait(cls, trait_type: type[StmtTrait]) -> bool:
        for trait in cls.traits:
            if isinstance(trait, trait_type):
                return True
        return False

    TraitType = TypeVar("TraitType", bound=StmtTrait)

    @classmethod
    def get_trait(cls, trait: type[TraitType]) -> TraitType | None:
        for t in cls.traits:
            if isinstance(t, trait):
                return t
        return None

    @classmethod
    def from_python_call(cls, state: LoweringState, node: ast.Call) -> Result:
        raise NotImplementedError

    def expect_one_result(self) -> ResultValue:
        if len(self._results) != 1:
            raise ValueError(f"expected one result, got {len(self._results)}")
        return self._results[0]

    # NOTE: statement should implement typecheck
    # this is done automatically via @statement, but
    # in the case manualy implementation is needed,
    # it should be implemented here.
    # NOTE: not an @abstractmethod to make linter happy
    def typecheck(self) -> None:
        raise NotImplementedError

    def verify(self) -> None:
        return
