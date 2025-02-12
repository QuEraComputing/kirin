from kirin import ir, types
from kirin.decl import info, statement

from ._dialect import dialect

T = types.TypeVar("T")


@statement
class BinOp(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    lhs: ir.SSAValue = info.argument(T, print=False)
    rhs: ir.SSAValue = info.argument(T, print=False)
    result: ir.ResultValue = info.result(T)


@statement(dialect=dialect)
class Add(BinOp):
    name = "add"


@statement(dialect=dialect)
class Sub(BinOp):
    name = "sub"


@statement(dialect=dialect)
class Mult(BinOp):
    name = "mult"


@statement(dialect=dialect)
class Div(BinOp):
    name = "div"


@statement(dialect=dialect)
class Mod(BinOp):
    name = "mod"


@statement(dialect=dialect)
class Pow(BinOp):
    name = "pow"


@statement(dialect=dialect)
class LShift(BinOp):
    name = "lshift"


@statement(dialect=dialect)
class RShift(BinOp):
    name = "rshift"


@statement(dialect=dialect)
class BitAnd(BinOp):
    name = "bitand"


@statement(dialect=dialect)
class BitOr(BinOp):
    name = "bitor"


@statement(dialect=dialect)
class BitXor(BinOp):
    name = "bitxor"


@statement(dialect=dialect)
class FloorDiv(BinOp):
    name = "floordiv"


@statement(dialect=dialect)
class MatMult(BinOp):
    name = "matmult"
