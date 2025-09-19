from typing import TYPE_CHECKING, Any

from kirin import ir, types, lowering
from kirin.decl import info, statement

if TYPE_CHECKING:
    from kirin.serialization.base.serializer import Serializer
    from kirin.serialization.base.deserializer import Deserializer

from ._dialect import dialect


@statement
class Cmp(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    lhs: ir.SSAValue = info.argument()
    rhs: ir.SSAValue = info.argument()
    result: ir.ResultValue = info.result(types.Bool)

    def serialize(self, serializer: "Serializer") -> dict[str, Any]:
        return {
            "lhs": serializer.serialize(self.lhs),
            "rhs": serializer.serialize(self.rhs),
            "result": serializer.serialize_result(self.result),
        }

    @classmethod
    def deserialize(cls, data: dict[str, Any], deserializer: "Deserializer") -> "Cmp":
        lhs = deserializer.deserialize(data["lhs"])
        rhs = deserializer.deserialize(data["rhs"])
        result = deserializer.deserialize_result(data["result"])
        stmt = cls(lhs=lhs, rhs=rhs)
        stmt.result = result
        return stmt


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
