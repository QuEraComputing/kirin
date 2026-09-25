from kirin import types, interp
from kirin.analysis import ForwardFrame, TypeInference

from . import stmts
from ._dialect import dialect

NUMBER = types.Int | types.Float


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(stmts.Add)
    @interp.impl(stmts.Sub)
    @interp.impl(stmts.Mult)
    @interp.impl(stmts.Mod)
    @interp.impl(stmts.FloorDiv)
    @interp.impl(stmts.Pow)
    def arithmetic(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: stmts.BinOp,
    ):
        """Type an arithmetic operation on two numbers.

        The result is a float if either operand is a float, else an int. A bool
        counts as an int, since `True + True` is `2`. Any other operand falls
        back to type resolution.
        """
        lhs, rhs = frame.get(stmt.lhs), frame.get(stmt.rhs)
        if lhs is types.Bottom or rhs is types.Bottom:
            return (types.Bottom,)
        if not (lhs.is_subseteq(NUMBER) and rhs.is_subseteq(NUMBER)):
            return interp_.eval_fallback(frame, stmt)
        if lhs.is_subseteq(types.Float) or rhs.is_subseteq(types.Float):
            return (types.Float,)
        return (types.Int,)

    @interp.impl(stmts.Div)
    def divf(self, typeinfer_, frame, stmt):
        return (types.Float,)

    @interp.impl(stmts.BitAnd)
    @interp.impl(stmts.BitOr)
    @interp.impl(stmts.BitXor)
    def bitwise(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: stmts.BinOp,
    ):
        """Type a bitwise operation on two ints, which is a bool for two bools."""
        lhs, rhs = frame.get(stmt.lhs), frame.get(stmt.rhs)
        if lhs is types.Bottom or rhs is types.Bottom:
            return (types.Bottom,)
        if lhs.is_subseteq(types.Bool) and rhs.is_subseteq(types.Bool):
            return (types.Bool,)
        if lhs.is_subseteq(types.Int) and rhs.is_subseteq(types.Int):
            return (types.Int,)
        return interp_.eval_fallback(frame, stmt)

    @interp.impl(stmts.LShift, types.Int)
    def lshift(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.RShift, types.Int)
    def rshift(self, interp, frame, stmt):
        return (types.Int,)

    @interp.impl(stmts.MatMult)
    def mat_mult(self, interp, frame, stmt):
        raise NotImplementedError("np.array @ np.array not implemented")
