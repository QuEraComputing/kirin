from typing import TypeVar
from functools import cached_property
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

    @cached_property
    def _hashable(self) -> bool:
        try:
            # try to hash the attribute
            hash(self.data)
            return True
        except TypeError:
            # if not hashable, compare by identity
            return False

    @cached_property
    def _hash_value(self) -> int:
        # calculate hash value if hashable
        if self._hashable:
            return hash((self.type, self.data))
        else:
            return hash((self.type, id(self.data)))

    def __init__(self, data: T, pytype: TypeAttribute | None = None):
        self.data = data

        if pytype is None:
            self.type = PyClass(type(data))
        else:
            self.type = pytype

    def __hash__(self) -> int:
        # use cached hash value
        return self._hash_value

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, PyAttr):
            return False

        if self._hashable:
            return self.type == value.type and self.data == value.data
        else:
            # if not hashable, compare by identity
            return self.type == value.type and self.data is value.data

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))
        with printer.rich(style="comment"):
            printer.plain_print(" : ")
            printer.print(self.type)

    def unwrap(self) -> T:
        return self.data
