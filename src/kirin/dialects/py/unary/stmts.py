from kirin import ir
from kirin.decl import info, statement

from ._dialect import dialect

T = ir.types.TypeVar("T")


@statement
class UnaryOp(ir.Statement):
    traits = frozenset({ir.Pure()})
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
