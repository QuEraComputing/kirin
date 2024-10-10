from dataclasses import dataclass
from typing import Generic, TypeVar

from kirin.codegen import CodeGen, DialectEmit, impl
from kirin.dialects.py.types.base import PyClass, PyType
from kirin.ir.attrs import Attribute
from kirin.ir.dialect import Dialect
from kirin.print.printer import Printer

dialect = Dialect("py.data")

T = TypeVar("T", covariant=True)


@dialect.register
@dataclass
class PyAttr(Generic[T], Attribute):
    name = "PyAttr"
    data: T
    type: PyType

    def __init__(self, data: T, pytype: PyType | None = None):
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


@dialect.register(key="dict")
@dataclass
class EmitDict(DialectEmit):
    @impl(PyAttr)
    def emit_PyAttr(self, emit: CodeGen, stmt: PyAttr):
        return {
            "name": stmt.name,
            "data": repr(stmt.data),
            "type": emit.emit_Attribute(stmt.type),
        }
