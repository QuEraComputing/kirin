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
        return ("(" + ", ".join(frame.get_values(stmt.args)) + ")",)
