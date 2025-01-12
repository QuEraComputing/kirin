from kirin import interp
from kirin.emit.julia import EmitJulia, EmitStrFrame

from . import stmts
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(stmts.Add)
    def emit_Add(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Add):
        return emit.emit_binaryop(frame, "+", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Sub)
    def emit_Sub(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Sub):
        return emit.emit_binaryop(frame, "-", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Mult)
    def emit_Mult(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Mult):
        return emit.emit_binaryop(frame, "*", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Div)
    def emit_Div(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Div):
        return emit.emit_binaryop(frame, "/", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Mod)
    def emit_Mod(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Mod):
        return emit.emit_binaryop(frame, "%", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(stmts.Pow)
    def emit_Pow(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Pow):
        return emit.emit_binaryop(frame, "^", stmt.lhs, stmt.rhs, stmt.result)
