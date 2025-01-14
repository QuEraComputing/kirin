from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin.print import Printer

from .abc import Attribute
from .types import PyClass, TypeAttribute

T = TypeVar("T")


@dataclass
class PyAttr(Generic[T], Attribute):
    name = "PyAttr"
    data: T
    type: TypeAttribute

    def __init__(self, data: T, pytype: TypeAttribute | None = None):
        self.data = data

        if pytype is None:
            self.type = PyClass(type(data))
        else:
            self.type = pytype

    def __hash__(self):
        return hash(self.data)

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))
        with printer.rich(style=printer.color.comment):
            printer.plain_print(" : ")
            printer.print(self.type)
