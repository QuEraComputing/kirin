from __future__ import annotations

from kirin import emit, interp

from .stmts import Range
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaMethodTable(interp.MethodTable):

    @interp.impl(Range)
    def range(self, emit_: emit.Julia, frame: emit.JuliaFrame, node: Range):
        start = frame.get(node.start)
        stop = frame.get(node.stop)
        step = frame.get(node.step)
        frame.write_line(f"{frame.ssa[node.result]} = {start}:{step}:{stop}")
        return (frame.ssa[node.result],)
