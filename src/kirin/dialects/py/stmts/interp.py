from typing import Any

from kirin.interp import Frame, Result, MethodTable, impl

from . import _stmts as py
from .dialect import dialect


@dialect.register
class PyMethodTable(MethodTable):

    @impl(py.Alias)
    def alias(self, interp, frame: Frame, stmt: py.Alias) -> Result[Any]:
        return (frame.get(stmt.value),)

    @impl(py.Constant)
    def constant(self, interp, frame: Frame, stmt: py.Constant) -> Result[Any]:
        return (stmt.value,)

    @impl(py.NewTuple)
    def new_tuple(self, interp, frame: Frame, stmt) -> Result[Any]:
        return (tuple(frame.get_values(stmt.args)),)

    @impl(py.NewList)
    def new_list(self, interp, frame: Frame, stmt: py.NewList) -> Result[Any]:
        return (list(frame.get_values(stmt.values)),)

    @impl(py.Append)
    def append(self, interp, frame: Frame, stmt: py.Append) -> Result[Any]:
        lst: list = frame.get(stmt.lst)
        val = frame.get(stmt.value)
        lst.append(val)
        return ()

    @impl(py.Len)
    def len(self, interp, frame: Frame, stmt: py.Len) -> Result[Any]:
        return (len(frame.get(stmt.value)),)

    @impl(py.Add)
    def add(self, interp, frame: Frame, stmt: py.Add) -> Result[Any]:
        return (frame.get(stmt.lhs) + frame.get(stmt.rhs),)

    @impl(py.Sub)
    def sub(self, interp, frame: Frame, stmt: py.Sub) -> Result[Any]:
        return (frame.get(stmt.lhs) - frame.get(stmt.rhs),)

    @impl(py.Mult)
    def mult(self, interp, frame: Frame, stmt: py.Mult) -> Result[Any]:
        return (frame.get(stmt.lhs) * frame.get(stmt.rhs),)

    @impl(py.Div)
    def div(self, interp, frame: Frame, stmt: py.Div) -> Result[Any]:
        return (frame.get(stmt.lhs) / frame.get(stmt.rhs),)

    @impl(py.Mod)
    def mod(self, interp, frame: Frame, stmt: py.Mod) -> Result[Any]:
        return (frame.get(stmt.lhs) % frame.get(stmt.rhs),)

    @impl(py.UAdd)
    def uadd(self, interp, frame: Frame, stmt: py.UAdd) -> Result[Any]:
        return (+frame.get(stmt.value),)

    @impl(py.USub)
    def usub(self, interp, frame: Frame, stmt: py.USub) -> Result[Any]:
        return (-frame.get(stmt.value),)

    @impl(py.Eq)
    def eq(self, interp, frame: Frame, stmt: py.Eq) -> Result[Any]:
        return (frame.get(stmt.lhs) == frame.get(stmt.rhs),)

    @impl(py.NotEq)
    def not_eq(self, interp, frame: Frame, stmt: py.NotEq) -> Result[Any]:
        return (frame.get(stmt.lhs) != frame.get(stmt.rhs),)

    @impl(py.Lt)
    def lt(self, interp, frame: Frame, stmt: py.Lt) -> Result[Any]:
        return (frame.get(stmt.lhs) < frame.get(stmt.rhs),)

    @impl(py.LtE)
    def lt_eq(self, interp, frame: Frame, stmt: py.LtE) -> Result[Any]:
        return (frame.get(stmt.lhs) <= frame.get(stmt.rhs),)

    @impl(py.Gt)
    def gt(self, interp, frame: Frame, stmt: py.Gt) -> Result[Any]:
        return (frame.get(stmt.lhs) > frame.get(stmt.rhs),)

    @impl(py.GtE)
    def gt_eq(self, interp, frame: Frame, stmt: py.GtE) -> Result[Any]:
        return (frame.get(stmt.lhs) >= frame.get(stmt.rhs),)

    @impl(py.And)
    def and_(self, interp, frame: Frame, stmt: py.And) -> Result[Any]:
        return (frame.get(stmt.lhs) and frame.get(stmt.rhs),)

    @impl(py.Or)
    def or_(self, interp, frame: Frame, stmt: py.Or) -> Result[Any]:
        return (frame.get(stmt.lhs) or frame.get(stmt.rhs),)

    @impl(py.Not)
    def not_(self, interp, frame: Frame, stmt: py.Not) -> Result[Any]:
        return (not frame.get(stmt.value),)

    @impl(py.BitAnd)
    def bit_and(self, interp, frame: Frame, stmt: py.BitAnd) -> Result[Any]:
        return (frame.get(stmt.lhs) & frame.get(stmt.rhs),)

    @impl(py.BitOr)
    def bit_or(self, interp, frame: Frame, stmt: py.BitOr) -> Result[Any]:
        return (frame.get(stmt.lhs) | frame.get(stmt.rhs),)

    @impl(py.BitXor)
    def bit_xor(self, interp, frame: Frame, stmt: py.BitXor) -> Result[Any]:
        return (frame.get(stmt.lhs) ^ frame.get(stmt.rhs),)

    @impl(py.Invert)
    def invert(self, interp, frame: Frame, stmt: py.Invert) -> Result[Any]:
        return (~frame.get(stmt.value),)

    @impl(py.Abs)
    def abs(self, interp, frame: Frame, stmt: py.Abs) -> Result[Any]:
        return (abs(frame.get(stmt.value)),)

    @impl(py.LShift)
    def lshift(self, interp, frame: Frame, stmt: py.LShift) -> Result[Any]:
        return (frame.get(stmt.lhs) << frame.get(stmt.rhs),)

    @impl(py.RShift)
    def rshift(self, interp, frame: Frame, stmt: py.RShift) -> Result[Any]:
        return (frame.get(stmt.lhs) >> frame.get(stmt.rhs),)

    @impl(py.FloorDiv)
    def floor_div(self, interp, frame: Frame, stmt: py.FloorDiv) -> Result[Any]:
        return (frame.get(stmt.lhs) // frame.get(stmt.rhs),)

    @impl(py.Pow)
    def pow(self, interp, frame: Frame, stmt: py.Pow) -> Result[Any]:
        return (frame.get(stmt.lhs) ** frame.get(stmt.rhs),)

    @impl(py.MatMult)
    def mat_mult(self, interp, frame: Frame, stmt: py.MatMult) -> Result[Any]:
        return (frame.get(stmt.lhs) @ frame.get(stmt.rhs),)

    @impl(py.GetItem)
    def getindex(self, interp, frame: Frame, stmt: py.GetItem) -> Result[Any]:
        return (frame.get(stmt.obj)[frame.get(stmt.index)],)

    @impl(py.SetItem)
    def setindex(self, interp, frame: Frame, stmt: py.SetItem) -> Result[Any]:
        frame.get(stmt.obj)[frame.get(stmt.index)] = frame.get(stmt.value)
        return (None,)

    @impl(py.Is)
    def is_(self, interp, frame: Frame, stmt: py.Is) -> Result[Any]:
        return (frame.get(stmt.lhs) is frame.get(stmt.rhs),)

    @impl(py.IsNot)
    def is_not(self, interp, frame: Frame, stmt: py.IsNot) -> Result[Any]:
        return (frame.get(stmt.lhs) is not frame.get(stmt.rhs),)

    @impl(py.In)
    def in_(self, interp, frame: Frame, stmt: py.In) -> Result[Any]:
        return (frame.get(stmt.lhs) in frame.get(stmt.rhs),)

    @impl(py.NotIn)
    def not_in(self, interp, frame: Frame, stmt: py.NotIn) -> Result[Any]:
        return (frame.get(stmt.lhs) not in frame.get(stmt.rhs),)

    @impl(py.GetAttr)
    def getattr(self, interp, frame: Frame, stmt: py.GetAttr) -> Result[Any]:
        return (getattr(frame.get(stmt.obj), stmt.attrname),)

    @impl(py.Range)
    def _range(self, interp, frame: Frame, stmt: py.Range) -> Result[Any]:
        return (range(*frame.get_values(stmt.args)),)

    @impl(py.Slice)
    def _slice(self, interp, frame: Frame, stmt: py.Slice) -> Result[Any]:
        start, stop, step = frame.get_values(stmt.args)
        if start is None and step is None:
            return (slice(stop),)
        elif step is None:
            return (slice(start, stop),)
        else:
            return (slice(start, stop, step),)
