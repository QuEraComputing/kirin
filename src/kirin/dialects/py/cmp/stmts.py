from kirin import ir, types
from kirin.decl import info, statement

from ._dialect import dialect


@statement
class Cmp(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    lhs: ir.SSAValue = info.argument()
    rhs: ir.SSAValue = info.argument()
    result: ir.ResultValue = info.result(types.Bool)


@statement(dialect=dialect)
class Eq(Cmp):
    name = "eq"


@statement(dialect=dialect)
class NotEq(Cmp):
    name = "ne"


@statement(dialect=dialect)
class Lt(Cmp):
    name = "lt"


@statement(dialect=dialect)
class Gt(Cmp):
    name = "gt"


@statement(dialect=dialect)
class LtE(Cmp):
    name = "lte"


@statement(dialect=dialect)
class GtE(Cmp):
    name = "gte"


@statement(dialect=dialect)
class Is(Cmp):
    name = "is"


@statement(dialect=dialect)
class IsNot(Cmp):
    name = "is_not"


@statement(dialect=dialect)
class In(Cmp):
    name = "in"


@statement(dialect=dialect)
class NotIn(Cmp):
    name = "not_in"
