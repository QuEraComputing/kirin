from kirin import types, interp

from . import stmts
from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(stmts.UAdd)
    @interp.impl(stmts.USub)
    def uadd(
        self, interp, frame: interp.Frame[types.TypeAttribute], stmt: stmts.UnaryOp
    ):
        return (frame.get(stmt.value),)

    @interp.impl(stmts.Not)
    def not_(self, interp, frame, stmt: stmts.Not):
        return (types.Bool,)

    @interp.impl(stmts.Invert, types.Int)
    def invert(self, interp, frame, stmt):
        return (types.Int,)
