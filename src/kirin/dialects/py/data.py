from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin.ir import Dialect, Attribute, types
from kirin.interp import MethodTable, impl
from kirin.emit.julia import EmitJulia
from kirin.print.printer import Printer

dialect = Dialect("py.data")

T = TypeVar("T", covariant=True)


@dialect.register
@dataclass
class PyAttr(Generic[T], Attribute):
    name = "PyAttr"
    data: T
    type: types.TypeAttribute

    def __init__(self, data: T, pytype: types.TypeAttribute | None = None):
        self.data = data

        if pytype is None:
            self.type = types.PyClass(type(data))
        else:
            self.type = pytype

    def __hash__(self):
        return hash(self.data)

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))
        with printer.rich(style=printer.color.comment):
            printer.plain_print(" : ")
            printer.print(self.type)


@dialect.register(key="emit.julia")
class JuliaTable(MethodTable):

    @impl(PyAttr)
    def emit_PyAttr(self, emit: EmitJulia, attr: PyAttr):
        if isinstance(attr.data, (int, float)):
            return repr(attr.data)
        elif isinstance(attr.data, str):
            return f'"{attr.data}"'
        else:
            raise ValueError(f"unsupported type {type(attr.data)}")
