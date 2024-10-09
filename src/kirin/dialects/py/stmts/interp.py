from kirin.interp import DialectInterpreter, ResultValue, impl

from . import _stmts as py
from .dialect import dialect


@dialect.register
class PyInterpreter(DialectInterpreter):

    @impl(py.Alias)
    def alias(self, interp, stmt: py.Alias, values: tuple) -> ResultValue:
        return ResultValue(values[0])

    @impl(py.Constant)
    def constant(self, interp, stmt: py.Constant, values: tuple) -> ResultValue:
        return ResultValue(stmt.value)

    @impl(py.NewTuple)
    def new_tuple(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(tuple(values))

    @impl(py.NewList)
    def new_list(self, interp, stmt: py.NewList, values: tuple) -> ResultValue:
        return ResultValue(list(values))

    @impl(py.Append)
    def append(self, interp, stmt: py.Append, values: tuple) -> ResultValue:
        lst: list = values[0]
        val = values[1]
        lst.append(val)

        return ResultValue()

    @impl(py.Len)
    def len(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(len(values[0]))

    @impl(py.Add)
    def add(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] + values[1])

    @impl(py.Sub)
    def sub(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] - values[1])

    @impl(py.Mult)
    def mult(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] * values[1])

    @impl(py.Div)
    def div(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] / values[1])

    @impl(py.Mod)
    def mod(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] % values[1])

    @impl(py.UAdd)
    def uadd(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(+values[0])

    @impl(py.USub)
    def usub(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(-values[0])

    @impl(py.Eq)
    def eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] == values[1])

    @impl(py.NotEq)
    def not_eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] != values[1])

    @impl(py.Lt)
    def lt(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] < values[1])

    @impl(py.LtE)
    def lt_eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] <= values[1])

    @impl(py.Gt)
    def gt(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] > values[1])

    @impl(py.GtE)
    def gt_eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] >= values[1])

    @impl(py.And)
    def and_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] and values[1])

    @impl(py.Or)
    def or_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] or values[1])

    @impl(py.Not)
    def not_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(not values[0])

    @impl(py.BitAnd)
    def bit_and(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] & values[1])

    @impl(py.BitOr)
    def bit_or(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] | values[1])

    @impl(py.BitXor)
    def bit_xor(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] ^ values[1])

    @impl(py.Invert)
    def invert(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(~values[0])

    @impl(py.Abs)
    def abs(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(abs(values[0]))

    @impl(py.LShift)
    def lshift(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] << values[1])

    @impl(py.RShift)
    def rshift(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] >> values[1])

    @impl(py.FloorDiv)
    def floor_div(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] // values[1])

    @impl(py.Pow)
    def pow(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] ** values[1])

    @impl(py.MatMult)
    def mat_mult(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] @ values[1])

    @impl(py.GetItem)
    def getindex(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0][values[1]])

    @impl(py.SetItem)
    def setindex(self, interp, stmt, values: tuple) -> ResultValue:
        values[0][values[1]] = values[2]
        return ResultValue(None)

    @impl(py.Is)
    def is_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] is values[1])

    @impl(py.IsNot)
    def is_not(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] is not values[1])

    @impl(py.In)
    def in_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] in values[1])

    @impl(py.NotIn)
    def not_in(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0] not in values[1])

    @impl(py.GetAttr)
    def getattr(self, interp, stmt: py.GetAttr, values: tuple) -> ResultValue:
        return ResultValue(getattr(values[0], stmt.attrname))

    @impl(py.Range)
    def _range(self, interp, stmt: py.Range, values: tuple) -> ResultValue:
        return ResultValue(range(*values))
