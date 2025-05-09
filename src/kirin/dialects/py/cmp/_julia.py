from kirin import emit, interp

from .stmts import Eq
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaEmit(interp.MethodTable):
    @interp.impl(Eq)
    def add(self, emit_: emit.Julia, frame: emit.JuliaFrame, node: Eq):
        lhs = frame.get(node.lhs)
        rhs = frame.get(node.rhs)
        frame.write_line(f"{frame.ssa[node.result]} = ({lhs} == {rhs})")
        return (frame.ssa[node.result],)
