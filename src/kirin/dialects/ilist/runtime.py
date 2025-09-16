# TODO: replace with something faster
from typing import Any, Generic, TypeVar, overload
from dataclasses import dataclass
from collections.abc import Sequence

from kirin import ir, types
from kirin.print.printer import Printer
from kirin.serialization.base.serializermixin import SerializerMixin

T = TypeVar("T")
L = TypeVar("L")


@dataclass
class IList(SerializerMixin, ir.Data[Sequence[T]], Sequence[T], Generic[T, L]):
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
        printer.plain_print("IList(")
        printer.print_seq(
            self.data, delim=", ", prefix="[", suffix="]", emit=printer.plain_print
        )
        printer.plain_print(")")

    def serialize(self, serializer: Any = None) -> dict[str, Any]:
        def enc(x: Any) -> Any:
            if serializer is not None and hasattr(serializer, "serialize"):
                return serializer.serialize(x)
            if isinstance(x, SerializerMixin):
                return x.serialize(serializer)
            return x

        return {
            "kind": "ilist",
            "data": [enc(a) for a in self.data],
            "elem": enc(self.elem),
        }

    @classmethod
    def deserialize(cls, data: dict[str, Any], serializer: Any = None) -> "IList":
        raw_items = data["data"]
        raw_elem = data["elem"]

        def dec(x: Any) -> Any:
            if not isinstance(x, dict):
                return x
            if serializer is not None and hasattr(serializer, "deserialize"):
                return serializer.deserialize(x)
            return x

        items = [dec(x) for x in raw_items]
        elem = dec(raw_elem)
        return IList(items, elem=elem)
