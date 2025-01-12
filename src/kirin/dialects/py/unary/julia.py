from kirin import interp
from kirin.emit.julia import EmitJulia, EmitStrFrame

from . import stmts
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(stmts.Not)
    def emit_Not(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.Not):
        return (emit.write_assign(frame, stmt.result, f"!{frame.get(stmt.value)}"),)

    @interp.impl(stmts.USub)
    def emit_USub(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.USub):
        return (emit.write_assign(frame, stmt.result, f"-{frame.get(stmt.value)}"),)

    @interp.impl(stmts.UAdd)
    def emit_UAdd(self, emit: EmitJulia, frame: EmitStrFrame, stmt: stmts.UAdd):
        return (emit.write_assign(frame, stmt.result, f"+{frame.get(stmt.value)}"),)
