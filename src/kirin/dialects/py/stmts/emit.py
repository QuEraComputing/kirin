from kirin.interp import MethodTable, impl
from kirin.emit.julia import EmitJulia, EmitStrFrame
from kirin.dialects.py.data import PyAttr

from . import _stmts as stmts
from .dialect import dialect


@dialect.register(key="emit.julia")
class JuliaTable(MethodTable):

    @impl(stmts.Constant)
    def emit_Constant(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Constant):
        return (emit.emit_attribute(PyAttr(stmt.value)),)

    @impl(stmts.NewTuple)
    def emit_NewTuple(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.NewTuple):
        return (
            emit.write_assign(
                frame, stmt.result, "(" + ", ".join(frame.get_values(stmt.args)) + ")"
            ),
        )

    def _emit_binaryop(
        self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.BinOp, sym: str
    ):
        return (
            emit.write_assign(
                frame,
                stmt.result,
                f"{frame.get(stmt.lhs)} {sym} {frame.get(stmt.rhs)}",
            ),
        )

    @impl(stmts.Eq)
    def emit_Eq(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Eq):
        return self._emit_binaryop(emit, frame, stmt, "==")

    @impl(stmts.GtE)
    def emit_GtE(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.GtE):
        return self._emit_binaryop(emit, frame, stmt, ">=")

    @impl(stmts.LtE)
    def emit_LtE(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.LtE):
        return self._emit_binaryop(emit, frame, stmt, "<=")

    @impl(stmts.NotEq)
    def emit_NotEq(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.NotEq):
        return self._emit_binaryop(emit, frame, stmt, "!=")

    @impl(stmts.Gt)
    def emit_Gt(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Gt):
        return self._emit_binaryop(emit, frame, stmt, ">")

    @impl(stmts.Lt)
    def emit_Lt(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Lt):
        return self._emit_binaryop(emit, frame, stmt, "<")

    @impl(stmts.Add)
    def emit_Add(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Add):
        return self._emit_binaryop(emit, frame, stmt, "+")

    @impl(stmts.Sub)
    def emit_Sub(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Sub):
        return self._emit_binaryop(emit, frame, stmt, "-")

    @impl(stmts.Mult)
    def emit_Mult(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Mult):
        return self._emit_binaryop(emit, frame, stmt, "*")

    @impl(stmts.Div)
    def emit_Div(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Div):
        return self._emit_binaryop(emit, frame, stmt, "/")

    @impl(stmts.Mod)
    def emit_Mod(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Mod):
        return self._emit_binaryop(emit, frame, stmt, "%")

    @impl(stmts.Pow)
    def emit_Pow(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Pow):
        return self._emit_binaryop(emit, frame, stmt, "^")

    @impl(stmts.And)
    def emit_And(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.And):
        return self._emit_binaryop(emit, frame, stmt, "&&")

    @impl(stmts.Or)
    def emit_Or(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Or):
        return self._emit_binaryop(emit, frame, stmt, "||")

    @impl(stmts.Not)
    def emit_Not(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Not):
        return (emit.write_assign(frame, stmt.result, f"!{frame.get(stmt.value)}"),)

    @impl(stmts.USub)
    def emit_USub(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.USub):
        return (emit.write_assign(frame, stmt.result, f"-{frame.get(stmt.value)}"),)

    @impl(stmts.UAdd)
    def emit_UAdd(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.UAdd):
        return (emit.write_assign(frame, stmt.result, f"+{frame.get(stmt.value)}"),)
