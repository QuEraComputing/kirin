from kirin import ir
from kirin.decl import info, statement

from ._dialect import dialect

T = ir.types.TypeVar("T")


@statement(dialect=dialect)
class New(ir.Statement):
    name = "list"
    traits = frozenset({ir.FromPythonCall()})
    values: tuple[ir.SSAValue, ...] = info.argument(T)
    result: ir.ResultValue = info.result(ir.types.List[T])


@statement(dialect=dialect)
class Append(ir.Statement):
    name = "append"
    traits = frozenset({ir.FromPythonCall()})
    list_: ir.SSAValue = info.argument(ir.types.List[T])
    value: ir.SSAValue = info.argument(T)
