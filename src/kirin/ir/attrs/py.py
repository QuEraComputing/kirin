from typing import TypeVar
from dataclasses import dataclass

from kirin.print import Printer

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

    def __hash__(self):
        return hash(self.data) + hash(self.type)

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))
        with printer.rich(style="comment"):
            printer.plain_print(" : ")
            printer.print(self.type)

    def unwrap(self) -> T:
        return self.data
