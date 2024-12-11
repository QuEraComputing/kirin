from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement, types

T = types.TypeVar("T")


@statement
class UnaryOp(Statement):
    traits = frozenset({Pure()})
    value: SSAValue = info.argument(T, print=False)
    result: ResultValue = info.result(T)


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
