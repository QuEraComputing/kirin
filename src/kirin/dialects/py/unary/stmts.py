from kirin import ir, types
from kirin.decl import info, statement

from ._dialect import dialect

T = types.TypeVar("T")


@statement
class UnaryOp(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    value: ir.SSAValue = info.argument(T, print=False)
    result: ir.ResultValue = info.result(T)


@statement(dialect=dialect)
class UAdd(UnaryOp):
    name = "uadd"


@statement(dialect=dialect)
class USub(UnaryOp):
    name = "usub"


@statement(dialect=dialect)
class Not(UnaryOp):
    name = "not"


@statement(dialect=dialect)
class Invert(UnaryOp):
    name = "invert"
