# TODO: replace with something faster
from typing import Any, Generic, TypeVar, overload
from dataclasses import dataclass
from collections.abc import Sequence

from kirin import ir, types
from kirin.print.printer import Printer

T = TypeVar("T")
L = TypeVar("L")


@dataclass
class IList(ir.Data[Sequence[T]], Generic[T, L]):
    """A simple immutable list."""

    data: Sequence[T]
    elem: types.TypeAttribute = types.Any

    def __post_init__(self):
        self.type = types.Generic(IList, self.elem, types.Literal(len(self.data)))

    def __hash__(self) -> int:
        return id(self)  # do not hash the data

    def __len__(self) -> int:
        return len(self.data)

    @overload
    def __add__(self, other: "IList[T, Any]") -> "IList[T, Any]": ...

    @overload
    def __add__(self, other: list[T]) -> "IList[T, Any]": ...

    def __add__(self, other):
        if isinstance(other, list):
            return IList(list(self.data) + other, elem=self.elem)
        elif isinstance(other, IList):
            return IList(
                list(self.data) + list(other.data), elem=self.elem.join(other.elem)
            )
        else:
            raise TypeError(
                f"unsupported operand type(s) for +: 'IList' and '{type(other)}'"
            )

    @overload
    def __radd__(self, other: "IList[T, Any]") -> "IList[T, Any]": ...

    @overload
    def __radd__(self, other: list[T]) -> "IList[T, Any]": ...

    def __radd__(self, other):
        return IList(other + self.data)

    def __repr__(self) -> str:
        return f"IList({self.data})"

    def __str__(self) -> str:
        return f"IList({self.data})"

    def __iter__(self):
        return iter(self.data)

    @overload
    def __getitem__(self, index: slice) -> "IList[T, Any]": ...

    @overload
    def __getitem__(self, index: int) -> T: ...

    def __getitem__(self, index: int | slice) -> T | "IList[T, Any]":
        if isinstance(index, slice):
            return IList(self.data[index])
        return self.data[index]

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, IList):
            return False
        return self.data == value.data

    def unwrap(self) -> "IList[T, L]":
        return self

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("IList(")
        printer.print_seq(
            self.data, delim=", ", prefix="[", suffix="]", emit=printer.plain_print
        )
        printer.plain_print(")")
