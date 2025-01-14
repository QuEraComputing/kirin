from typing import Any

from kirin import ir
from kirin.decl import info, statement
from kirin.interp import Frame, Interpreter, MethodTable, impl

dialect = ir.Dialect("py.list")

ElemT = ir.types.TypeVar("ElemT")


@statement(dialect=dialect)
class Append(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    lst: ir.SSAValue = info.argument(ir.types.List[ElemT])
    value: ir.SSAValue = info.argument(ir.types.Any)


@dialect.register
class MutableListMethod(MethodTable):

    @impl(Append)
    def append(self, interp: Interpreter, frame: Frame[Any], stmt: Append):
        lst: list = frame.get(stmt.lst)
        value = frame.get(stmt.value)
        lst.append(value)
        return (lst,)
