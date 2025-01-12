from kirin import interp

from . import stmts
from ._dialect import dialect


@dialect.register
class PyMethodTable(interp.MethodTable):

    @interp.impl(stmts.Add)
    def add(self, interp, frame: interp.Frame, stmt: stmts.Add):
        return (frame.get(stmt.lhs) + frame.get(stmt.rhs),)

    @interp.impl(stmts.Sub)
    def sub(self, interp, frame: interp.Frame, stmt: stmts.Sub):
        return (frame.get(stmt.lhs) - frame.get(stmt.rhs),)

    @interp.impl(stmts.Mult)
    def mult(self, interp, frame: interp.Frame, stmt: stmts.Mult):
        return (frame.get(stmt.lhs) * frame.get(stmt.rhs),)

    @interp.impl(stmts.Div)
    def div(self, interp, frame: interp.Frame, stmt: stmts.Div):
        return (frame.get(stmt.lhs) / frame.get(stmt.rhs),)

    @interp.impl(stmts.Mod)
    def mod(self, interp, frame: interp.Frame, stmt: stmts.Mod):
        return (frame.get(stmt.lhs) % frame.get(stmt.rhs),)

    @interp.impl(stmts.BitAnd)
    def bit_and(self, interp, frame: interp.Frame, stmt: stmts.BitAnd):
        return (frame.get(stmt.lhs) & frame.get(stmt.rhs),)

    @interp.impl(stmts.BitOr)
    def bit_or(self, interp, frame: interp.Frame, stmt: stmts.BitOr):
        return (frame.get(stmt.lhs) | frame.get(stmt.rhs),)

    @interp.impl(stmts.BitXor)
    def bit_xor(self, interp, frame: interp.Frame, stmt: stmts.BitXor):
        return (frame.get(stmt.lhs) ^ frame.get(stmt.rhs),)

    @interp.impl(stmts.LShift)
    def lshift(self, interp, frame: interp.Frame, stmt: stmts.LShift):
        return (frame.get(stmt.lhs) << frame.get(stmt.rhs),)

    @interp.impl(stmts.RShift)
    def rshift(self, interp, frame: interp.Frame, stmt: stmts.RShift):
        return (frame.get(stmt.lhs) >> frame.get(stmt.rhs),)

    @interp.impl(stmts.FloorDiv)
    def floor_div(self, interp, frame: interp.Frame, stmt: stmts.FloorDiv):
        return (frame.get(stmt.lhs) // frame.get(stmt.rhs),)

    @interp.impl(stmts.Pow)
    def pow(self, interp, frame: interp.Frame, stmt: stmts.Pow):
        return (frame.get(stmt.lhs) ** frame.get(stmt.rhs),)

    @interp.impl(stmts.MatMult)
    def mat_mult(self, interp, frame: interp.Frame, stmt: stmts.MatMult):
        return (frame.get(stmt.lhs) @ frame.get(stmt.rhs),)
