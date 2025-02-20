from kirin import types, interp

from . import stmts
from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(stmts.Add, types.Float, types.Float)
    @interp.impl(stmts.Add, types.Float, types.Int)
    @interp.impl(stmts.Add, types.Int, types.Float)
    def addf(self, interp, frame, stmt):
        return (types.Float,)

    @interp.impl(stmts.Add, types.Int, types.Int)
    def addi(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.Sub, types.Float, types.Float)
    @interp.impl(stmts.Sub, types.Float, types.Int)
    @interp.impl(stmts.Sub, types.Int, types.Float)
    def subf(self, *_):
        return (types.Float,)

    @interp.impl(stmts.Sub, types.Int, types.Int)
    def subi(self, *_):
        return (types.Int,)

    @interp.impl(stmts.Mult, types.Float, types.Float)
    @interp.impl(stmts.Mult, types.Float, types.Int)
    @interp.impl(stmts.Mult, types.Int, types.Float)
    def multf(self, *_):
        return (types.Float,)

    @interp.impl(stmts.Mult, types.Int, types.Int)
    def multi(self, *_):
        return (types.Int,)

    @interp.impl(stmts.Div)
    def divf(self, typeinfer_, frame, stmt):
        return (types.Float,)

    @interp.impl(stmts.Mod, types.Float, types.Float)
    @interp.impl(stmts.Mod, types.Float, types.Int)
    @interp.impl(stmts.Mod, types.Int, types.Float)
    def modf(self, *_):
        return (types.Float,)

    @interp.impl(stmts.Mod, types.Int, types.Int)
    def modi(self, *_):
        return (types.Int,)

    @interp.impl(stmts.BitAnd, types.Int, types.Int)
    def bit_andi(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.BitAnd, types.Bool, types.Bool)
    def bit_andb(self, interp, frame, stmt):
        return (types.Bool,)

    @interp.impl(stmts.BitOr, types.Int, types.Int)
    def bit_ori(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.BitOr, types.Bool, types.Bool)
    def bit_orb(self, interp, frame, stmt):
        return (types.Bool,)

    @interp.impl(stmts.BitXor, types.Int, types.Int)
    def bit_xori(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.BitXor, types.Bool, types.Bool)
    def bit_xorb(self, interp, frame, stmt):
        return (types.Bool,)

    @interp.impl(stmts.LShift, types.Int)
    def lshift(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.RShift, types.Int)
    def rshift(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.FloorDiv, types.Float, types.Float)
    @interp.impl(stmts.FloorDiv, types.Int, types.Float)
    @interp.impl(stmts.FloorDiv, types.Float, types.Int)
    def floor_divf(self, interp, frame, stmt):
        return (types.Float,)

    @interp.impl(stmts.FloorDiv, types.Int, types.Int)
    def floor_divi(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.Pow, types.Float, types.Float)
    @interp.impl(stmts.Pow, types.Float, types.Int)
    @interp.impl(stmts.Pow, types.Int, types.Float)
    def powf(self, interp, frame, stmt):
        return (types.Float,)

    @interp.impl(stmts.Pow, types.Int, types.Int)
    def powi(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.MatMult)
    def mat_mult(self, interp, frame, stmt):
        raise NotImplementedError("np.array @ np.array not implemented")
