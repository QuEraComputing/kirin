import ast
from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin import ir, types, interp, lowering, exceptions
from kirin.emit.julia import EmitJulia
from kirin.print.printer import Printer

dialect = ir.Dialect("py.data")

T = TypeVar("T", covariant=True)


@dialect.register
@dataclass
class PyAttr(Generic[T], ir.Attribute):
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


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_Name(
        self, state: lowering.LoweringState, node: ast.Name
    ) -> lowering.Result:
        name = node.id
        if isinstance(node.ctx, ast.Load):
            value = state.current_frame.get(name)
            if value is None:
                raise exceptions.DialectLoweringError(f"{name} is not defined")
            return lowering.Result(value)
        elif isinstance(node.ctx, ast.Store):
            raise exceptions.DialectLoweringError("unhandled store operation")
        else:  # Del
            raise exceptions.DialectLoweringError("unhandled del operation")

    def lower_Expr(
        self, state: lowering.LoweringState, node: ast.Expr
    ) -> lowering.Result:
        return state.visit(node.value)


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(PyAttr)
    def emit_PyAttr(self, emit: EmitJulia, attr: PyAttr):
        if isinstance(attr.data, (int, float)):
            return repr(attr.data)
        elif isinstance(attr.data, str):
            return f'"{attr.data}"'
        else:
            raise ValueError(f"unsupported type {type(attr.data)}")
