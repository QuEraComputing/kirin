from __future__ import annotations

import re
from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, ClassVar
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin.print import Printer, Printable
from kirin.ir.attrs.abc import Attribute
from kirin.ir.attrs.types import AnyType, TypeAttribute

if TYPE_CHECKING:
    from kirin.ir.use import Use
    from kirin.ir.nodes.stmt import Statement
    from kirin.ir.nodes.block import Block


@dataclass
class SSAValue(ABC, Printable):
    """Base class for all SSA values in the IR."""

    type: TypeAttribute = field(default_factory=AnyType, init=False, repr=True)
    """The type of this SSA value."""
    hints: dict[str, Attribute] = field(default_factory=dict, init=False, repr=False)
    """Hints for this SSA value."""
    uses: set[Use] = field(init=False, default_factory=set, repr=False)
    """The uses of this SSA value."""
    _name: str | None = field(init=False, default=None, repr=True)
    """The name of this SSA value."""
    name_pattern: ClassVar[re.Pattern[str]] = re.compile(r"([A-Za-z_$.-][\w$.-]*)")
    """The pattern that the name of this SSA value must match."""

    @property
    @abstractmethod
    def owner(self) -> Statement | Block:
        """The object that owns this SSA value."""
        ...

    @property
    def name(self) -> str | None:
        """The name of this SSA value."""
        return self._name

    @name.setter
    def name(self, name: str | None) -> None:
        if name and not self.name_pattern.fullmatch(name):
            raise ValueError(f"Invalid name: {name}")
        self._name = name

    def __repr__(self) -> str:
        if self.name:
            return f"{type(self).__name__}({self.name})"
        return f"{type(self).__name__}({id(self)})"

    def __hash__(self) -> int:
        return id(self)

    def add_use(self, use: Use) -> Self:
        """Add a use to this SSA value."""
        self.uses.add(use)
        return self

    def remove_use(self, use: Use) -> Self:
        """Remove a use from this SSA value."""
        # print(use)
        # assert use in self.uses, "Use not found"
        if use in self.uses:
            self.uses.remove(use)
        return self

    def replace_by(self, other: SSAValue) -> None:
        """Replace this SSA value with another SSA value. Update all uses."""
        for use in self.uses.copy():
            use.stmt.args[use.index] = other

        if other.name is None and self.name is not None:
            other.name = self.name

        assert len(self.uses) == 0, "Uses not empty"

    # TODO: also delete BlockArgument from arglist
    def delete(self, safe: bool = True) -> None:
        """Delete this SSA value. If `safe` is `True`, raise an error if there are uses."""
        if safe and len(self.uses) > 0:
            raise ValueError("Cannot delete SSA value with uses")
        self.replace_by(DeletedSSAValue(self))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(printer.state.ssa_id[self])


@dataclass
class ResultValue(SSAValue):
    """SSAValue that is a result of a [`Statement`][kirin.ir.nodes.stmt.Statement]."""

    stmt: Statement = field(init=False)
    """The statement that this value is a result of."""
    index: int = field(init=False)
    """The index of this value in the statement's result list."""

    # NOTE: we will assign AnyType unless specified.
    #       when SSAValue is a ResultValue, the type is inferred
    #       later in the compilation process.
    def __init__(
        self, stmt: Statement, index: int, type: TypeAttribute | None = None
    ) -> None:
        super().__init__()
        self.type = type or AnyType()
        self.stmt = stmt
        self.index = index

    @property
    def owner(self) -> Statement:
        return self.stmt

    def __hash__(self) -> int:
        return id(self)

    def __repr__(self) -> str:
        if self.type is self.type.top():
            type_str = ""
        else:
            type_str = f"[{self.type}]"

        if self.name:
            return (
                f"<{type(self).__name__}{type_str} {self.name}, uses: {len(self.uses)}>"
            )
        return f"<{type(self).__name__}{type_str} stmt: {self.stmt.name}, uses: {len(self.uses)}>"


@dataclass
class BlockArgument(SSAValue):
    """SSAValue that is an argument to a [`Block`][kirin.ir.Block]."""

    block: Block = field(init=False)
    """The block that this argument belongs to."""
    index: int = field(init=False)
    """The index of this argument in the block's argument list."""

    def __init__(
        self, block: Block, index: int, type: TypeAttribute = AnyType()
    ) -> None:
        super().__init__()
        self.type = type
        self.block = block
        self.index = index

    @property
    def owner(self) -> Block:
        return self.block

    def __hash__(self) -> int:
        return id(self)

    def __repr__(self) -> str:
        if self.name:
            return f"<{type(self).__name__}[{self.type}] {self.name}, uses: {len(self.uses)}>"
        return f"<{type(self).__name__}[{self.type}] index: {self.index}, uses: {len(self.uses)}>"

    def print_impl(self, printer: Printer) -> None:
        super().print_impl(printer)
        if not isinstance(self.type, AnyType):
            with printer.rich(style="comment"):
                printer.plain_print(" : ")
                printer.print(self.type)


@dataclass
class DeletedSSAValue(SSAValue):
    value: SSAValue = field(init=False)

    def __init__(self, value: SSAValue) -> None:
        super().__init__()
        self.value = value
        self.type = value.type

    def __hash__(self) -> int:
        return id(self)

    def __repr__(self) -> str:
        return f"<{type(self).__name__}[{self.type}] value: {self.value}, uses: {len(self.uses)}>"

    @property
    def owner(self) -> Statement | Block:
        return self.value.owner


@dataclass
class TestValue(SSAValue):
    """Test SSAValue for testing IR construction."""

    def __init__(self, type: TypeAttribute = AnyType()) -> None:
        super().__init__()
        self.type = type

    def __hash__(self) -> int:
        return id(self)

    @property
    def owner(self) -> Statement | Block:
        raise NotImplementedError
