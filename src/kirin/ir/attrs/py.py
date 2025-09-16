from typing import Any, Type, TypeVar, cast
from dataclasses import dataclass

from kirin.print import Printer
from kirin.serialization.base.serializermixin import SerializerMixin

from .data import Data
from .types import PyClass, TypeAttribute

T = TypeVar("T")
U = TypeVar("U")  # used for deserialize return typing


@dataclass
class PyAttr(Data[T], SerializerMixin):
    """Python attribute for compile-time values.
    This is a generic attribute that holds a Python value.

    The constructor takes a Python value and an optional type attribute.
    If the type attribute is not provided, the type of the value is inferred
    as `PyClass(type(value))`.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    name = "PyAttr"
    data: T

    def __init__(self, data: T, pytype: TypeAttribute | None = None):
        self.data = data

        if pytype is None:
            self.type = PyClass(type(data))
        else:
            self.type = pytype

    def __hash__(self) -> int:
        return hash((self.type, self.data))

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, PyAttr):
            return False

        return self.type == value.type and self.data == value.data

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))
        with printer.rich(style="comment"):
            printer.plain_print(" : ")
            printer.print(self.type)

    def unwrap(self) -> T:
        return self.data

    def serialize(self, serializer) -> dict[str, Any]:
        if isinstance(self.data, SerializerMixin):
            return {"data": self.data.serialize(serializer)}
        return {"data": self.data}

    @classmethod
    def deserialize(cls: Type["PyAttr[U]"], data: Any, serializer) -> "PyAttr[U]":
        payload = (
            data.get("data") if isinstance(data, dict) and "data" in data else data
        )
        if (
            isinstance(payload, dict)
            and serializer is not None
            and hasattr(serializer, "deserialize")
        ):
            payload = serializer.deserialize(payload)
        return cls(
            cast(U, payload)
        )  # cast payload to U so the constructor call type-checks
