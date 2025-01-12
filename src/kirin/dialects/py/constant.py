import ast
from typing import Generic, TypeVar

from kirin import ir, interp, lowering, exceptions
from kirin.decl import info, statement
from kirin.print import Printer
from kirin.emit.julia import EmitJulia, EmitStrFrame
from kirin.dialects.py.data import PyAttr

dialect = ir.Dialect("py.constant")

T = TypeVar("T", covariant=True)


@statement(dialect=dialect)
class Constant(ir.Statement, Generic[T]):
    name = "constant"
    traits = frozenset({ir.Pure(), ir.ConstantLike(), ir.FromPythonCall()})
    value: T = info.attribute(property=True)
    result: ir.ResultValue = info.result()

    # NOTE: we allow py.Constant take data.PyAttr too
    def __init__(self, value: T | PyAttr[T]) -> None:
        if not isinstance(value, PyAttr):
            value = PyAttr(value)
        super().__init__(
            properties={"value": value},
            result_types=(value.type,),
        )

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        printer.plain_print(repr(self.value))
        with printer.rich(style=printer.color.comment):
            printer.plain_print(" : ")
            printer.print(self.result.type)

    def typecheck(self) -> None:
        if not isinstance(self.result.type, ir.types.TypeAttribute):
            raise exceptions.VerificationError(
                self, f"Expected result type to be PyType, got {self.result.type}"
            )


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Constant(
        self, state: lowering.LoweringState, node: ast.Constant
    ) -> lowering.Result:
        return lowering.Result(state.append_stmt(Constant(node.value)))


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Constant)
    def constant(self, interp, frame: interp.Frame, stmt: Constant):
        return (stmt.value,)


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(Constant)
    def emit_Constant(self, emit: EmitJulia, frame: EmitStrFrame, stmt: Constant):
        return (emit.emit_attribute(PyAttr(stmt.value)),)
