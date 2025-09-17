from typing import Any, Type, TypeVar, cast
from dataclasses import dataclass

from kirin.print import Printer
from kirin.serialization.base.serializermixin import SerializerMixin

from .data import Data
from .types import PyClass, TypeAttribute

T = TypeVar("T")


@dataclass
class PyAttr(Data[T]):
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
        out = {
            "kind": "attribute-pyattr",
            "module": self.__class__.__module__,
            "name": self.__class__.__name__,
            "data": {},
        }
        if isinstance(self.data, SerializerMixin):
            data = {
                "data": self.data.serialize(serializer),
                "pytype": self.type.serialize(serializer),
            }
            out.update(data)
        else:
            data = {
                "data": serializer.serialize(self.data),
                "pytype": self.type.serialize(serializer),
            }
            out.update(data)
        return out

    @classmethod
    def deserialize(cls: Type["PyAttr[T]"], data: Any, serializer) -> "PyAttr[T]":
        payload = (
            data.get("data") if isinstance(data, dict) and "data" in data else data
        )
        pytype = None
        if isinstance(data, dict) and "pytype" in data and serializer is not None:
            pytype = serializer.deserialize(data["pytype"])
        if (
            isinstance(payload, dict)
            and serializer is not None
            and hasattr(serializer, "deserialize")
        ):
            payload = serializer.deserialize(payload)
        return cls(
            cast(T, payload),
            pytype=pytype,
        )
