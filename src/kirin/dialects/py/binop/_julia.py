from kirin import emit, interp

from .stmts import Add
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaEmit(interp.MethodTable):
    @interp.impl(Add)
    def add(self, emit_: emit.Julia, frame: emit.JuliaFrame, node: Add):
        lhs = frame.get(node.lhs)
        rhs = frame.get(node.rhs)
        frame.write_line(f"{frame.ssa[node.result]} = ({lhs} + {rhs})")
        return (frame.ssa[node.result],)
