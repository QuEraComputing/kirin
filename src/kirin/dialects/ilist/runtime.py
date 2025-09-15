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

    def serialize(self) -> dict[str, Any]:
        def enc(x: Any) -> Any:
            if isinstance(x, SerializerMixin):
                return x.serialize()
            return x

        return {"data": [enc(a) for a in self.data], "elem": enc(self.elem)}

    @classmethod
    def deserialize(cls, data: dict[str, Any]) -> "IList":
        items = []
        for x in data.get("data", []):
            items.append(x)
        # 'elem' is expected to be already decoded by the global Serializer.
        # Do not call .deserialize() here (that caused the AttributeError).
        elem = data.get("elem", types.Any)
        return IList(items, elem=elem)
