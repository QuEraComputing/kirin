# TODO: replace with something faster
from typing import Any, Generic, TypeVar, overload
from dataclasses import dataclass
from collections.abc import Sequence

from kirin import ir, types
from kirin.print.printer import Printer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.base.deserializer import Deserializer
from kirin.serialization.core.serializationunit import SerializationUnit

from ._dialect import dialect

T = TypeVar("T")
L = TypeVar("L")


@dataclass
@dialect.register
class IList(ir.Data[Sequence[T]], Sequence[T], Generic[T, L]):
    """A simple immutable list."""

    name = "IList"
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
    def __getitem__(self, index: int) -> T: ...

    @overload
    def __getitem__(self, index: slice) -> "IList[T, Any]": ...

    def __getitem__(self, index: int | slice) -> T | "IList[T, Any]":
        if isinstance(index, slice):
            return IList(self.data[index])
        return self.data[index]

    def __contains__(self, item: object) -> bool:
        return item in self.data

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, IList):
            return False
        return self.data == value.data

    def unwrap(self) -> Sequence[T]:
        return self

    def print_impl(self, printer: Printer) -> None:
        printer.print_seq(
            self.data, delim=", ", prefix="[", suffix="]", emit=printer.plain_print
        )
        printer.plain_print(")")

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        return (
            isinstance(other, IList)
            and self.data == other.data
            and self.elem.is_structurally_equal(other.elem, context=context)
        )

    def serialize(self, serializer: "Serializer") -> "SerializationUnit":
        return SerializationUnit(
            kind="ilist",
            module_name=dialect.name,
            class_name=IList.__name__,
            data={
                "data": serializer.serialize(self.data),
                "elem": serializer.serialize_attribute(self.elem),
            },
        )

    @classmethod
    def deserialize(
        cls, serUnit: "SerializationUnit", deserializer: "Deserializer"
    ) -> "IList":
        items = deserializer.deserialize(serUnit.data["data"])
        elem = deserializer.deserialize(serUnit.data["elem"])
        return IList(items, elem=elem)
