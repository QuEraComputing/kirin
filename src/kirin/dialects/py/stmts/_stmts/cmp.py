from kirin.ir import Pure, ResultValue, types
from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect
from kirin.dialects.py.stmts._stmts.binop import BinOp


@statement
class Cmp(BinOp):
    traits = frozenset({Pure()})
    result: ResultValue = info.result(types.Bool)


@statement(dialect=dialect)
class Eq(Cmp):
    name = "eq"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class NotEq(Cmp):
    name = "ne"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Lt(Cmp):
    name = "lt"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Gt(Cmp):
    name = "gt"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class LtE(Cmp):
    name = "lte"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class GtE(Cmp):
    name = "gte"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Is(Cmp):
    name = "is"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class IsNot(Cmp):
    name = "is_not"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class In(Cmp):
    name = "in"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class NotIn(Cmp):
    name = "not_in"
    traits = frozenset({Pure()})
