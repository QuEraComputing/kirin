from kirin import interp

from . import stmts
from ._dialect import dialect


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(stmts.UAdd)
    def uadd(self, interp, frame: interp.Frame, stmt: stmts.UAdd):
        return (+frame.get(stmt.value),)

    @interp.impl(stmts.USub)
    def usub(self, interp, frame: interp.Frame, stmt: stmts.USub):
        return (-frame.get(stmt.value),)

    @interp.impl(stmts.Not)
    def not_(self, interp, frame: interp.Frame, stmt: stmts.Not):
        return (not frame.get(stmt.value),)

    @interp.impl(stmts.Invert)
    def invert(self, interp, frame: interp.Frame, stmt: stmts.Invert):
        return (~frame.get(stmt.value),)
