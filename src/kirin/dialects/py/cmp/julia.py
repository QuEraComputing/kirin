from kirin import interp
from kirin.emit.julia import EmitJulia, EmitStrFrame

from . import stmts
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(stmts.Eq)
    def emit_Eq(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Eq):
        return emit.emit_binaryop(frame, "==", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.GtE)
    def emit_GtE(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.GtE):
        return emit.emit_binaryop(frame, ">=", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.LtE)
    def emit_LtE(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.LtE):
        return emit.emit_binaryop(frame, "<=", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.NotEq)
    def emit_NotEq(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.NotEq):
        return emit.emit_binaryop(frame, "!=", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Gt)
    def emit_Gt(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Gt):
        return emit.emit_binaryop(frame, ">", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Lt)
    def emit_Lt(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Lt):
        return emit.emit_binaryop(frame, "<", stmt.lhs, stmt.rhs, stmt.result)
