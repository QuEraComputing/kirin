from kirin.ir import Pure, SSAValue, ResultValue, types
from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect

from .binop import BinOp


@statement
class BoolOp(BinOp):
    traits = frozenset({Pure()})
    lhs: SSAValue = info.argument(print=False)
    rhs: SSAValue = info.argument(print=False)
    result: ResultValue = info.result(types.Bool)


@statement(dialect=dialect)
class And(BoolOp):
    name = "and"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Or(BoolOp):
    name = "or"
    traits = frozenset({Pure()})
