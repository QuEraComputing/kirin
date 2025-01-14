from kirin import interp

from . import stmts as cmp
from ._dialect import dialect


@dialect.register
class CmpMethod(interp.MethodTable):

    @interp.impl(cmp.Eq)
    def eq(self, interp, frame: interp.Frame, stmt: cmp.Eq):
        return (frame.get(stmt.lhs) == frame.get(stmt.rhs),)

    @interp.impl(cmp.NotEq)
    def not_eq(self, interp, frame: interp.Frame, stmt: cmp.NotEq):
        return (frame.get(stmt.lhs) != frame.get(stmt.rhs),)

    @interp.impl(cmp.Lt)
    def lt(self, interp, frame: interp.Frame, stmt: cmp.Lt):
        return (frame.get(stmt.lhs) < frame.get(stmt.rhs),)

    @interp.impl(cmp.LtE)
    def lt_eq(self, interp, frame: interp.Frame, stmt: cmp.LtE):
        return (frame.get(stmt.lhs) <= frame.get(stmt.rhs),)

    @interp.impl(cmp.Gt)
    def gt(self, interp, frame: interp.Frame, stmt: cmp.Gt):
        return (frame.get(stmt.lhs) > frame.get(stmt.rhs),)

    @interp.impl(cmp.GtE)
    def gt_eq(self, interp, frame: interp.Frame, stmt: cmp.GtE):
        return (frame.get(stmt.lhs) >= frame.get(stmt.rhs),)

    @interp.impl(cmp.In)
    def in_(self, interp, frame: interp.Frame, stmt: cmp.In):
        return (frame.get(stmt.lhs) in frame.get(stmt.rhs),)

    @interp.impl(cmp.NotIn)
    def not_in(self, interp, frame: interp.Frame, stmt: cmp.NotIn):
        return (frame.get(stmt.lhs) not in frame.get(stmt.rhs),)

    @interp.impl(cmp.Is)
    def is_(self, interp, frame: interp.Frame, stmt: cmp.Is):
        return (frame.get(stmt.lhs) is frame.get(stmt.rhs),)

    @interp.impl(cmp.IsNot)
    def is_not(self, interp, frame: interp.Frame, stmt: cmp.IsNot):
        return (frame.get(stmt.lhs) is not frame.get(stmt.rhs),)
