from typing import Any

from kirin.interp import Result, DialectInterpreter, impl

from . import _stmts as py
from .dialect import dialect


@dialect.register
class PyInterpreter(DialectInterpreter):

    @impl(py.Alias)
    def alias(self, interp, stmt: py.Alias, values: tuple) -> Result[Any]:
        return (values[0],)

    @impl(py.Constant)
    def constant(self, interp, stmt: py.Constant, values: tuple) -> Result[Any]:
        return (stmt.value,)

    @impl(py.NewTuple)
    def new_tuple(self, interp, stmt, values: tuple) -> Result[Any]:
        return (tuple(values),)

    @impl(py.NewList)
    def new_list(self, interp, stmt: py.NewList, values: tuple) -> Result[Any]:
        return (list(values),)

    @impl(py.Append)
    def append(self, interp, stmt: py.Append, values: tuple) -> Result[Any]:
        lst: list = values[0]
        val = values[1]
        lst.append(val)
        return ()

    @impl(py.Len)
    def len(self, interp, stmt, values: tuple) -> Result[Any]:
        return (len(values[0]),)

    @impl(py.Add)
    def add(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] + values[1],)

    @impl(py.Sub)
    def sub(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] - values[1],)

    @impl(py.Mult)
    def mult(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] * values[1],)

    @impl(py.Div)
    def div(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] / values[1],)

    @impl(py.Mod)
    def mod(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] % values[1],)

    @impl(py.UAdd)
    def uadd(self, interp, stmt, values: tuple) -> Result[Any]:
        return (+values[0],)

    @impl(py.USub)
    def usub(self, interp, stmt, values: tuple) -> Result[Any]:
        return (-values[0],)

    @impl(py.Eq)
    def eq(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] == values[1],)

    @impl(py.NotEq)
    def not_eq(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] != values[1],)

    @impl(py.Lt)
    def lt(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] < values[1],)

    @impl(py.LtE)
    def lt_eq(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] <= values[1],)

    @impl(py.Gt)
    def gt(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] > values[1],)

    @impl(py.GtE)
    def gt_eq(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] >= values[1],)

    @impl(py.And)
    def and_(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] and values[1],)

    @impl(py.Or)
    def or_(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] or values[1],)

    @impl(py.Not)
    def not_(self, interp, stmt, values: tuple) -> Result[Any]:
        return (not values[0],)

    @impl(py.BitAnd)
    def bit_and(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] & values[1],)

    @impl(py.BitOr)
    def bit_or(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] | values[1],)

    @impl(py.BitXor)
    def bit_xor(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] ^ values[1],)

    @impl(py.Invert)
    def invert(self, interp, stmt, values: tuple) -> Result[Any]:
        return (~values[0],)

    @impl(py.Abs)
    def abs(self, interp, stmt, values: tuple) -> Result[Any]:
        return (abs(values[0]),)

    @impl(py.LShift)
    def lshift(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] << values[1],)

    @impl(py.RShift)
    def rshift(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] >> values[1],)

    @impl(py.FloorDiv)
    def floor_div(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] // values[1],)

    @impl(py.Pow)
    def pow(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] ** values[1],)

    @impl(py.MatMult)
    def mat_mult(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] @ values[1],)

    @impl(py.GetItem)
    def getindex(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0][values[1]],)

    @impl(py.SetItem)
    def setindex(self, interp, stmt, values: tuple) -> Result[Any]:
        values[0][values[1]] = values[2]
        return (None,)

    @impl(py.Is)
    def is_(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] is values[1],)

    @impl(py.IsNot)
    def is_not(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] is not values[1],)

    @impl(py.In)
    def in_(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] in values[1],)

    @impl(py.NotIn)
    def not_in(self, interp, stmt, values: tuple) -> Result[Any]:
        return (values[0] not in values[1],)

    @impl(py.GetAttr)
    def getattr(self, interp, stmt: py.GetAttr, values: tuple) -> Result[Any]:
        return (getattr(values[0], stmt.attrname),)

    @impl(py.Range)
    def _range(self, interp, stmt: py.Range, values: tuple) -> Result[Any]:
        return (range(*values),)

    @impl(py.Slice)
    def _slice(self, interp, stmt: py.Slice, values: tuple) -> Result[Any]:
        start, stop, step = values
        if start is None and step is None:
            return (slice(stop),)
        elif step is None:
            return (slice(start, stop),)
        else:
            return (slice(start, stop, step),)
