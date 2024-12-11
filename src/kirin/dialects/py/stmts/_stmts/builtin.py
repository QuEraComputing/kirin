from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement, types

T = types.TypeVar("T", bound=types.Int | types.Float)


@statement(dialect=dialect)
class Abs(Statement):
    name = "builtin.abs"
    traits = frozenset({Pure()})
    value: SSAValue = info.argument(T, print=False)
    result: ResultValue = info.result(T)
