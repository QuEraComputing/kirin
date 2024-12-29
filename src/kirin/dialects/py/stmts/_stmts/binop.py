from kirin.ir import Pure, SSAValue, Statement, ResultValue, types
from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect

T = types.TypeVar("T")


@statement
class BinOp(Statement):
    traits = frozenset({Pure()})
    lhs: SSAValue = info.argument(T, print=False)
    rhs: SSAValue = info.argument(T, print=False)
    result: ResultValue = info.result(T)


@statement(dialect=dialect)
class Add(BinOp):
    name = "add"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Sub(BinOp):
    name = "sub"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Mult(BinOp):
    name = "mult"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Div(BinOp):
    name = "div"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Mod(BinOp):
    name = "mod"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class Pow(BinOp):
    name = "pow"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class LShift(BinOp):
    name = "lshift"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class RShift(BinOp):
    name = "rshift"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class BitAnd(BinOp):
    name = "bitand"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class BitOr(BinOp):
    name = "bitor"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class BitXor(BinOp):
    name = "bitxor"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class FloorDiv(BinOp):
    name = "floordiv"
    traits = frozenset({Pure()})


@statement(dialect=dialect)
class MatMult(BinOp):
    name = "matmult"
    traits = frozenset({Pure()})
