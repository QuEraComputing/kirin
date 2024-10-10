from __future__ import annotations

import re
from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, ClassVar

from typing_extensions import Self

from kirin.ir.attrs import AnyType, TypeAttribute
from kirin.ir.derive import derive, field
from kirin.print import Printable, Printer

if TYPE_CHECKING:
    from kirin.ir.nodes.block import Block
    from kirin.ir.nodes.stmt import Statement
    from kirin.ir.use import Use


@derive(id_hash=True, init=True)
class SSAValue(ABC, Printable):
    type: TypeAttribute = field(default_factory=AnyType, init=False, repr=True)
    uses: set[Use] = field(init=False, default_factory=set, repr=False)
    _name: str | None = field(init=False, default=None, repr=True)
    name_pattern: ClassVar[re.Pattern[str]] = re.compile(r"([A-Za-z_$.-][\w$.-]*)")

    @property
    @abstractmethod
    def owner(self) -> Statement | Block: ...

    @property
    def name(self) -> str | None:
        return self._name

    @name.setter
    def name(self, name: str) -> None:
        if not self.name_pattern.fullmatch(name):
            raise ValueError(f"Invalid name: {name}")
        self._name = name

    def __repr__(self) -> str:
        if self.name:
            return f"{type(self).__name__}({self.name})"
        return f"{type(self).__name__}({id(self)})"

    def add_use(self, use: Use) -> Self:
        self.uses.add(use)
        return self

    def remove_use(self, use: Use) -> Self:
        # print(use)
        # assert use in self.uses, "Use not found"
        if use in self.uses:
            self.uses.remove(use)
        return self

    def replace_by(self, other: SSAValue) -> None:
        for use in self.uses.copy():
            use.stmt.args[use.index] = other

        if other.name is None and self.name is not None:
            other.name = self.name

        assert len(self.uses) == 0, "Uses not empty"

    def delete(self, safe: bool = True) -> None:
        if safe and len(self.uses) > 0:
            raise ValueError("Cannot delete SSA value with uses")
        self.replace_by(DeletedSSAValue(self))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(printer.state.ssa_id[self])


@derive(id_hash=True)
class ResultValue(SSAValue):
    stmt: Statement = field(init=False)
    index: int = field(init=False)

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

    def __repr__(self) -> str:
        if self.type.is_top():
            type_str = ""
        else:
            type_str = f"[{self.type}]"

        if self.name:
            return (
                f"<{type(self).__name__}{type_str} {self.name}, uses: {len(self.uses)}>"
            )
        return f"<{type(self).__name__}{type_str} stmt: {self.stmt.name}, uses: {len(self.uses)}>"


@derive(id_hash=True)
class BlockArgument(SSAValue):
    block: Block = field(init=False)
    index: int = field(init=False)

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

    def __repr__(self) -> str:
        if self.name:
            return f"<{type(self).__name__}[{self.type}] {self.name}, uses: {len(self.uses)}>"
        return f"<{type(self).__name__}[{self.type}] index: {self.index}, uses: {len(self.uses)}>"

    def print_impl(self, printer: Printer) -> None:
        super().print_impl(printer)
        if not isinstance(self.type, AnyType):
            with printer.rich(style=printer.color.comment):
                printer.plain_print(" : ")
                printer.print(self.type)


@derive(id_hash=True)
class DeletedSSAValue(SSAValue):
    value: SSAValue = field(init=False)

    def __init__(self, value: SSAValue) -> None:
        super().__init__()
        self.value = value
        self.type = value.type

    def __repr__(self) -> str:
        return f"<{type(self).__name__}[{self.type}] value: {self.value}, uses: {len(self.uses)}>"

    @property
    def owner(self) -> Statement | Block:
        return self.value.owner


@derive(id_hash=True)
class TestValue(SSAValue):
    """Test SSAValue for testing IR construction."""

    def __init__(self, type: TypeAttribute = AnyType()) -> None:
        super().__init__()
        self.type = type

    @property
    def owner(self) -> Statement | Block:
        raise NotImplementedError
