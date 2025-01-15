from kirin import types, interp
from kirin.dialects.py.binop import Add

from .stmts import New, Append
from ._dialect import dialect


@dialect.register
class ListMethods(interp.MethodTable):

    @interp.impl(New)
    def new(self, interp, frame: interp.Frame, stmt: New):
        return (list(frame.get_values(stmt.values)),)

    @interp.impl(Add, types.PyClass(list), types.PyClass(list))
    def add(self, interp, frame: interp.Frame, stmt: Add):
        return (frame.get(stmt.lhs) + frame.get(stmt.rhs),)

    @interp.impl(Append)
    def append(self, interp, frame: interp.Frame, stmt: Append):
        return (frame.get(stmt.list_).append(frame.get(stmt.value)),)
