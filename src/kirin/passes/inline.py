from typing import Callable
from dataclasses import field, dataclass

from kirin import ir
from kirin.passes import Pass
from kirin.rewrite import Walk, Inline, Fixpoint, CFGCompactify, DeadCodeElimination
from kirin.rewrite.abc import RewriteResult


def aggresive(x: ir.IRNode) -> bool:
    return True


@dataclass
class InlinePass(Pass):
    herustic: Callable[[ir.IRNode], bool] = field(default=aggresive)

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:

        result = Walk(Inline(heuristic=self.herustic)).rewrite(mt.code)
        result = Walk(CFGCompactify()).rewrite(mt.code).join(result)

        # dce
        dce = DeadCodeElimination()
        return Fixpoint(Walk(dce)).rewrite(mt.code).join(result)
