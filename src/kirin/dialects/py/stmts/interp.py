from typing import Any

from kirin.interp import Frame, MethodTable, StatementResult, impl

from . import _stmts as py
from .dialect import dialect


@dialect.register
class PyMethodTable(MethodTable):

    @impl(py.Alias)
    def alias(self, interp, frame: Frame, stmt: py.Alias) -> StatementResult[Any]:
        return (frame.get(stmt.value),)

    @impl(py.Constant)
    def constant(self, interp, frame: Frame, stmt: py.Constant) -> StatementResult[Any]:
        return (stmt.value,)

    @impl(py.NewTuple)
    def new_tuple(self, interp, frame: Frame, stmt) -> StatementResult[Any]:
        return (tuple(frame.get_values(stmt.args)),)

    @impl(py.NewList)
    def new_list(self, interp, frame: Frame, stmt: py.NewList) -> StatementResult[Any]:
        return (list(frame.get_values(stmt.values)),)

    @impl(py.Append)
    def append(self, interp, frame: Frame, stmt: py.Append) -> StatementResult[Any]:
        lst: list = frame.get(stmt.lst)
        val = frame.get(stmt.value)
        lst.append(val)
        return ()

    @impl(py.Len)
    def len(self, interp, frame: Frame, stmt: py.Len) -> StatementResult[Any]:
        return (len(frame.get(stmt.value)),)

    @impl(py.Add)
    def add(self, interp, frame: Frame, stmt: py.Add) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) + frame.get(stmt.rhs),)

    @impl(py.Sub)
    def sub(self, interp, frame: Frame, stmt: py.Sub) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) - frame.get(stmt.rhs),)

    @impl(py.Mult)
    def mult(self, interp, frame: Frame, stmt: py.Mult) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) * frame.get(stmt.rhs),)

    @impl(py.Div)
    def div(self, interp, frame: Frame, stmt: py.Div) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) / frame.get(stmt.rhs),)

    @impl(py.Mod)
    def mod(self, interp, frame: Frame, stmt: py.Mod) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) % frame.get(stmt.rhs),)

    @impl(py.UAdd)
    def uadd(self, interp, frame: Frame, stmt: py.UAdd) -> StatementResult[Any]:
        return (+frame.get(stmt.value),)

    @impl(py.USub)
    def usub(self, interp, frame: Frame, stmt: py.USub) -> StatementResult[Any]:
        return (-frame.get(stmt.value),)

    @impl(py.Eq)
    def eq(self, interp, frame: Frame, stmt: py.Eq) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) == frame.get(stmt.rhs),)

    @impl(py.NotEq)
    def not_eq(self, interp, frame: Frame, stmt: py.NotEq) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) != frame.get(stmt.rhs),)

    @impl(py.Lt)
    def lt(self, interp, frame: Frame, stmt: py.Lt) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) < frame.get(stmt.rhs),)

    @impl(py.LtE)
    def lt_eq(self, interp, frame: Frame, stmt: py.LtE) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) <= frame.get(stmt.rhs),)

    @impl(py.Gt)
    def gt(self, interp, frame: Frame, stmt: py.Gt) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) > frame.get(stmt.rhs),)

    @impl(py.GtE)
    def gt_eq(self, interp, frame: Frame, stmt: py.GtE) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) >= frame.get(stmt.rhs),)

    @impl(py.And)
    def and_(self, interp, frame: Frame, stmt: py.And) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) and frame.get(stmt.rhs),)

    @impl(py.Or)
    def or_(self, interp, frame: Frame, stmt: py.Or) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) or frame.get(stmt.rhs),)

    @impl(py.Not)
    def not_(self, interp, frame: Frame, stmt: py.Not) -> StatementResult[Any]:
        return (not frame.get(stmt.value),)

    @impl(py.BitAnd)
    def bit_and(self, interp, frame: Frame, stmt: py.BitAnd) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) & frame.get(stmt.rhs),)

    @impl(py.BitOr)
    def bit_or(self, interp, frame: Frame, stmt: py.BitOr) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) | frame.get(stmt.rhs),)

    @impl(py.BitXor)
    def bit_xor(self, interp, frame: Frame, stmt: py.BitXor) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) ^ frame.get(stmt.rhs),)

    @impl(py.Invert)
    def invert(self, interp, frame: Frame, stmt: py.Invert) -> StatementResult[Any]:
        return (~frame.get(stmt.value),)

    @impl(py.Abs)
    def abs(self, interp, frame: Frame, stmt: py.Abs) -> StatementResult[Any]:
        return (abs(frame.get(stmt.value)),)

    @impl(py.LShift)
    def lshift(self, interp, frame: Frame, stmt: py.LShift) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) << frame.get(stmt.rhs),)

    @impl(py.RShift)
    def rshift(self, interp, frame: Frame, stmt: py.RShift) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) >> frame.get(stmt.rhs),)

    @impl(py.FloorDiv)
    def floor_div(
        self, interp, frame: Frame, stmt: py.FloorDiv
    ) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) // frame.get(stmt.rhs),)

    @impl(py.Pow)
    def pow(self, interp, frame: Frame, stmt: py.Pow) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) ** frame.get(stmt.rhs),)

    @impl(py.MatMult)
    def mat_mult(self, interp, frame: Frame, stmt: py.MatMult) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) @ frame.get(stmt.rhs),)

    @impl(py.GetItem)
    def getindex(self, interp, frame: Frame, stmt: py.GetItem) -> StatementResult[Any]:
        return (frame.get(stmt.obj)[frame.get(stmt.index)],)

    @impl(py.SetItem)
    def setindex(self, interp, frame: Frame, stmt: py.SetItem) -> StatementResult[Any]:
        frame.get(stmt.obj)[frame.get(stmt.index)] = frame.get(stmt.value)
        return (None,)

    @impl(py.Is)
    def is_(self, interp, frame: Frame, stmt: py.Is) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) is frame.get(stmt.rhs),)

    @impl(py.IsNot)
    def is_not(self, interp, frame: Frame, stmt: py.IsNot) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) is not frame.get(stmt.rhs),)

    @impl(py.In)
    def in_(self, interp, frame: Frame, stmt: py.In) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) in frame.get(stmt.rhs),)

    @impl(py.NotIn)
    def not_in(self, interp, frame: Frame, stmt: py.NotIn) -> StatementResult[Any]:
        return (frame.get(stmt.lhs) not in frame.get(stmt.rhs),)

    @impl(py.GetAttr)
    def getattr(self, interp, frame: Frame, stmt: py.GetAttr) -> StatementResult[Any]:
        return (getattr(frame.get(stmt.obj), stmt.attrname),)

    @impl(py.Range)
    def _range(self, interp, frame: Frame, stmt: py.Range) -> StatementResult[Any]:
        return (range(*frame.get_values(stmt.args)),)

    @impl(py.Slice)
    def _slice(self, interp, frame: Frame, stmt: py.Slice) -> StatementResult[Any]:
        start, stop, step = frame.get_values(stmt.args)
        if start is None and step is None:
            return (slice(stop),)
        elif step is None:
            return (slice(start, stop),)
        else:
            return (slice(start, stop, step),)
