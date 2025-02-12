from kirin import ir, types
from kirin.decl import info, statement

from ._dialect import dialect

T = types.TypeVar("T")


@statement(dialect=dialect)
class New(ir.Statement):
    name = "list"
    traits = frozenset({ir.FromPythonCall()})
    values: tuple[ir.SSAValue, ...] = info.argument(T)
    result: ir.ResultValue = info.result(types.List[T])


@statement(dialect=dialect)
class Append(ir.Statement):
    name = "append"
    traits = frozenset({ir.FromPythonCall()})
    list_: ir.SSAValue = info.argument(types.List[T])
    value: ir.SSAValue = info.argument(T)
